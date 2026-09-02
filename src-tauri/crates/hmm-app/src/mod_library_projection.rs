use crate::mod_library_projection_tracking::ModLibraryProjectionFreshnessGuard;
use crate::mod_library_query::{normalize_text, validated_status_map};
use crate::{
    InstallManifestStatus, InstallManifestStatusSummary, ModLibraryProfileContext,
    ModLibraryQueryError, ModLibraryService, ModLibraryStatusProvider,
};
use hmm_ports::{
    ModLibraryProfileProjection, ModLibraryProjectionQueryError, ModLibraryProjectionQueryRequest,
    ModLibraryProjectionRepository, ModLibraryProjectionSnapshot, ModLibraryProjectionState,
    ModLibraryProjectionStatus, ModLibraryProjectionStatusRecord,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

pub struct ModLibraryProjectionRefreshService {
    library_service: Arc<ModLibraryService>,
    status_provider: Arc<dyn ModLibraryStatusProvider>,
    repository: Arc<dyn ModLibraryProjectionRepository>,
    freshness_guard: Arc<ModLibraryProjectionFreshnessGuard>,
    refresh_lock: Mutex<()>,
}

impl ModLibraryProjectionRefreshService {
    pub fn new(
        library_service: Arc<ModLibraryService>,
        status_provider: Arc<dyn ModLibraryStatusProvider>,
        repository: Arc<dyn ModLibraryProjectionRepository>,
        freshness_guard: Arc<ModLibraryProjectionFreshnessGuard>,
    ) -> Self {
        Self {
            library_service,
            status_provider,
            repository,
            freshness_guard,
            refresh_lock: Mutex::new(()),
        }
    }

    pub(crate) fn ensure(
        &self,
        profile_context: Option<&ModLibraryProfileContext>,
    ) -> Result<(ModLibraryProjectionState, Option<String>), ModLibraryQueryError> {
        let _activity_guard = self
            .freshness_guard
            .begin_refresh()
            .map_err(|_| ModLibraryQueryError::LibraryUnavailable)?;
        if self.freshness_guard.global_is_unavailable() {
            return Err(ModLibraryQueryError::LibraryUnavailable);
        }
        let _guard = self
            .refresh_lock
            .lock()
            .map_err(|_| ModLibraryQueryError::LibraryUnavailable)?;
        let mut records = None;
        let mut state = self
            .repository
            .state()
            .map_err(|_| ModLibraryQueryError::LibraryUnavailable)?;
        if !state.is_complete_for(state.source_fingerprint.as_deref().unwrap_or_default())
            || state.source_fingerprint.is_none()
        {
            self.repository
                .mark_dirty(None)
                .map_err(|_| ModLibraryQueryError::LibraryUnavailable)?;
            let current_records = self
                .library_service
                .get_mod_library_projection_records()
                .map_err(|_| ModLibraryQueryError::LibraryUnavailable)?;
            let source_fingerprint = fingerprint_records(&current_records)?;
            state = self
                .repository
                .rebuild(&ModLibraryProjectionSnapshot {
                    source_fingerprint,
                    records: current_records.clone(),
                    profiles: Vec::new(),
                })
                .map_err(|_| ModLibraryQueryError::LibraryUnavailable)?;
            records = Some(current_records);
        }

        let profile_fingerprint = if let Some(context) = profile_context {
            if self
                .freshness_guard
                .profile_is_unavailable(&context.profile_id)
            {
                return Err(ModLibraryQueryError::StatusUnavailable);
            }
            let current_profile = self
                .repository
                .profile_state(&context.profile_id)
                .map_err(|_| ModLibraryQueryError::StatusUnavailable)?;
            if let Some(profile) = current_profile {
                if profile.readiness == hmm_ports::ModLibraryProjectionReadiness::Complete
                    && profile.source_fingerprint.is_some()
                {
                    profile.source_fingerprint
                } else {
                    Some(self.rebuild_profile(context, &mut records)?)
                }
            } else {
                Some(self.rebuild_profile(context, &mut records)?)
            }
        } else {
            None
        };

        Ok((state, profile_fingerprint))
    }

    fn rebuild_profile(
        &self,
        context: &ModLibraryProfileContext,
        records: &mut Option<Vec<hmm_ports::ModLibraryProjectionRecord>>,
    ) -> Result<String, ModLibraryQueryError> {
        self.repository
            .mark_profile_dirty(&context.profile_id, None)
            .map_err(|_| ModLibraryQueryError::StatusUnavailable)?;
        let current_records = match records.take() {
            Some(records) => records,
            None => self
                .library_service
                .get_mod_library_projection_records()
                .map_err(|_| ModLibraryQueryError::StatusUnavailable)?,
        };
        let mod_ids = current_records
            .iter()
            .map(|record| record.mod_id.clone())
            .collect::<Vec<_>>();
        let summaries = self
            .status_provider
            .query_statuses(context, &mod_ids)
            .map_err(|_| ModLibraryQueryError::StatusUnavailable)?;
        let summaries_by_mod_id = validated_status_map(context, &mod_ids, summaries)
            .map_err(|_| ModLibraryQueryError::StatusUnavailable)?;
        let mut statuses = Vec::new();
        for mod_id in &mod_ids {
            let summary = summaries_by_mod_id
                .get(mod_id.as_str())
                .ok_or(ModLibraryQueryError::StatusUnavailable)?;
            if let Some(status) = projection_status(summary)? {
                statuses.push(ModLibraryProjectionStatusRecord {
                    mod_id: summary.mod_id.clone(),
                    status,
                    managed_file_count: summary.managed_file_count as u64,
                    backup_count: summary.backup_count as u64,
                });
            }
        }
        let fingerprint = fingerprint_summaries(&summaries_by_mod_id);
        self.repository
            .replace_profile(&ModLibraryProfileProjection {
                profile_id: context.profile_id.clone(),
                source_fingerprint: fingerprint.clone(),
                statuses,
            })
            .map_err(|_| ModLibraryQueryError::StatusUnavailable)?;
        self.freshness_guard.clear_profile(&context.profile_id);
        Ok(fingerprint)
    }
}

pub(crate) fn projection_query_request(
    query: &crate::ModLibraryQuery,
    state: &ModLibraryProjectionState,
    profile_fingerprint: Option<String>,
) -> Result<ModLibraryProjectionQueryRequest, ModLibraryQueryError> {
    let filter = match &query.filter {
        crate::ModLibraryFilter::All => hmm_ports::ModLibraryProjectionQueryFilter::All,
        crate::ModLibraryFilter::Status(status) => {
            hmm_ports::ModLibraryProjectionQueryFilter::Status(status_to_query_status(*status))
        }
        crate::ModLibraryFilter::Category(category_id) => {
            hmm_ports::ModLibraryProjectionQueryFilter::Category(category_id.clone())
        }
    };
    let profile = match (&query.profile_context, profile_fingerprint) {
        (Some(context), Some(source_fingerprint)) => {
            Some(hmm_ports::ModLibraryProjectionProfileQuery {
                profile_id: context.profile_id.clone(),
                source_fingerprint,
            })
        }
        (Some(_), None) => return Err(ModLibraryQueryError::StatusUnavailable),
        (None, None) => None,
        (None, Some(_)) => return Err(ModLibraryQueryError::LibraryUnavailable),
    };
    let Some(source_fingerprint) = state.source_fingerprint.clone() else {
        return Err(ModLibraryQueryError::LibraryUnavailable);
    };
    Ok(ModLibraryProjectionQueryRequest {
        source_fingerprint,
        profile,
        normalized_search: normalize_text(&query.search),
        filter,
        page: query.page,
        page_size: query.page_size,
    })
}

pub(crate) fn map_projection_query_error(
    error: ModLibraryProjectionQueryError,
) -> ModLibraryQueryError {
    match error {
        ModLibraryProjectionQueryError::Unavailable => ModLibraryQueryError::LibraryUnavailable,
        ModLibraryProjectionQueryError::CategoryNotFound => ModLibraryQueryError::CategoryNotFound,
        ModLibraryProjectionQueryError::ProfileUnavailable => {
            ModLibraryQueryError::StatusUnavailable
        }
    }
}

fn projection_status(
    summary: &InstallManifestStatusSummary,
) -> Result<Option<ModLibraryProjectionStatus>, ModLibraryQueryError> {
    match summary.status {
        InstallManifestStatus::NotInstalled => Ok(None),
        InstallManifestStatus::Installed => Ok(Some(ModLibraryProjectionStatus::Installed)),
        InstallManifestStatus::CommittedCleanupPending => {
            Ok(Some(ModLibraryProjectionStatus::CommittedCleanupPending))
        }
        InstallManifestStatus::CleanupPending => {
            Ok(Some(ModLibraryProjectionStatus::CleanupPending))
        }
        InstallManifestStatus::RollbackRequired => {
            Ok(Some(ModLibraryProjectionStatus::RollbackRequired))
        }
        InstallManifestStatus::RepairRequired => {
            Ok(Some(ModLibraryProjectionStatus::RepairRequired))
        }
        InstallManifestStatus::Unknown => Err(ModLibraryQueryError::StatusUnavailable),
    }
}

fn status_to_query_status(
    status: InstallManifestStatus,
) -> hmm_ports::ModLibraryProjectionQueryStatus {
    match status {
        InstallManifestStatus::NotInstalled => {
            hmm_ports::ModLibraryProjectionQueryStatus::NotInstalled
        }
        InstallManifestStatus::Installed => hmm_ports::ModLibraryProjectionQueryStatus::Installed,
        InstallManifestStatus::CommittedCleanupPending => {
            hmm_ports::ModLibraryProjectionQueryStatus::CommittedCleanupPending
        }
        InstallManifestStatus::CleanupPending => {
            hmm_ports::ModLibraryProjectionQueryStatus::CleanupPending
        }
        InstallManifestStatus::RollbackRequired => {
            hmm_ports::ModLibraryProjectionQueryStatus::RollbackRequired
        }
        InstallManifestStatus::RepairRequired => {
            hmm_ports::ModLibraryProjectionQueryStatus::RepairRequired
        }
        InstallManifestStatus::Unknown => hmm_ports::ModLibraryProjectionQueryStatus::Unknown,
    }
}

fn fingerprint_records(
    records: &[hmm_ports::ModLibraryProjectionRecord],
) -> Result<String, ModLibraryQueryError> {
    let mut records = records.to_vec();
    records.sort_by(|left, right| left.mod_id.as_str().cmp(right.mod_id.as_str()));
    let mut hasher = Sha256::new();
    for record in records {
        hash_text(&mut hasher, record.mod_id.as_str());
        hash_text(&mut hasher, record.display_revision_id.as_str());
        hash_text(&mut hasher, &record.package_id);
        hash_text(&mut hasher, &record.display_name);
        hash_optional_text(&mut hasher, record.author.as_deref());
        hash_optional_text(&mut hasher, record.version_label.as_deref());
        hash_optional_text(&mut hasher, record.external_import_adapter_id.as_deref());
        hash_text(&mut hasher, &record.size_label);
        hash_text(
            &mut hasher,
            &serde_json::to_string(&record.preview_image)
                .map_err(|_| ModLibraryQueryError::LibraryUnavailable)?,
        );
        let mut labels = record.labels;
        labels.sort_by(|left, right| {
            left.category_id
                .cmp(&right.category_id)
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.color.cmp(&right.color))
        });
        for label in labels {
            hash_optional_text(&mut hasher, label.category_id.as_deref());
            hash_text(&mut hasher, &label.name);
            hash_optional_text(&mut hasher, label.color.as_deref());
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn fingerprint_summaries(
    summaries: &std::collections::HashMap<String, InstallManifestStatusSummary>,
) -> String {
    let mut keys = summaries.keys().collect::<Vec<_>>();
    keys.sort();
    let mut hasher = Sha256::new();
    for key in keys {
        let summary = &summaries[key];
        hash_text(&mut hasher, key);
        hash_text(&mut hasher, status_to_query_status(summary.status).as_str());
        hash_text(&mut hasher, &summary.managed_file_count.to_string());
        hash_text(&mut hasher, &summary.backup_count.to_string());
    }
    format!("{:x}", hasher.finalize())
}

fn hash_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn hash_optional_text(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hash_text(hasher, value);
        }
        None => hasher.update([0]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{ModId, ModRevisionId, PreviewImageRejectionReason};
    use hmm_ports::{
        ModLibraryProjectionLabel, ModLibraryProjectionRecord, StoredImportPreviewImage,
    };

    #[test]
    fn global_fingerprint_is_stable_when_category_pair_order_varies() {
        let make_record = |labels| ModLibraryProjectionRecord {
            mod_id: ModId::new("mod-a"),
            display_revision_id: ModRevisionId::new("revision-a"),
            package_id: "package-a".to_owned(),
            display_name: "Alpha".to_owned(),
            author: None,
            version_label: None,
            size_label: "1 B".to_owned(),
            preview_image: StoredImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::Missing,
            },
            external_import_adapter_id: None,
            labels,
        };
        let armor = ModLibraryProjectionLabel {
            category_id: Some("category-armor".to_owned()),
            name: "Armor".to_owned(),
            color: None,
        };
        let cosmetic = ModLibraryProjectionLabel {
            category_id: None,
            name: "Cosmetic".to_owned(),
            color: Some("blue".to_owned()),
        };

        assert_eq!(
            fingerprint_records(&[make_record(vec![armor.clone(), cosmetic.clone()])])
                .expect("fingerprint"),
            fingerprint_records(&[make_record(vec![cosmetic, armor])]).expect("fingerprint")
        );
    }
}
