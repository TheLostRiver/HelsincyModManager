use hmm_core::{
    classify_reinstall_targets, resolve_installed_revision, FileLayer, GameId, InstallFileProvider,
    InstallManifest, InstallManifestEntry, InstallManifestStatusConsumption,
    InstallManifestValidationError, InstallPlan, InstallTargetPath, InstalledFileSummary, ModId,
    ModRevisionId, PackageFileId, ProfileId, ReinstallClassificationError, ReinstallManifestError,
    ReinstallTargetClass, ReinstallTargetState,
};
use hmm_ports::{
    InstallBackupStore, InstallGameFileSystem, InstallManifestRepository,
    ModImportResultRepository, ReinstallRecoveryTransactionRepository, StoredModRevision,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinstallPreviewRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub candidate_revision_id: ModRevisionId,
    pub layer: FileLayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReinstallPreviewStatus {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinstallRevisionSummary {
    pub revision_id: ModRevisionId,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReinstallTargetCounts {
    pub retained: usize,
    pub replaced: usize,
    pub added: usize,
    pub stale: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReinstallBlockingReason {
    NotInstalled,
    CandidateNotFound,
    CandidateNotReady,
    CandidateOwnerMismatch,
    CandidateAlreadyInstalled,
    ManifestStateUnsafe,
    InstalledRevisionUnknown,
    SourceUnavailable,
    TargetMissing,
    TargetChanged,
    TargetReadFailed,
    BackupMissing,
    BackupReadFailed,
    PlanConflict,
    CrossModTargetConflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinstallBlockingReasonSummary {
    pub reason: ReinstallBlockingReason,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReinstallPlanPreview {
    pub status: ReinstallPreviewStatus,
    pub installed_revision: Option<ReinstallRevisionSummary>,
    pub candidate_revision: Option<ReinstallRevisionSummary>,
    pub counts: ReinstallTargetCounts,
    pub blocking_reasons: Vec<ReinstallBlockingReasonSummary>,
    pub plan_token: Option<String>,
}

impl ReinstallPlanPreview {
    fn blocked(
        installed_revision: Option<ModRevisionId>,
        candidate_revision: Option<ModRevisionId>,
        reason: ReinstallBlockingReason,
    ) -> Self {
        Self {
            status: ReinstallPreviewStatus::Blocked,
            installed_revision: installed_revision.map(revision_summary),
            candidate_revision: candidate_revision.map(revision_summary),
            counts: ReinstallTargetCounts::default(),
            blocking_reasons: vec![ReinstallBlockingReasonSummary { reason, count: 1 }],
            plan_token: None,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReinstallCandidatePlanError {
    #[error("candidate revision is not ready")]
    NotReady,
    #[error("candidate install plan is unavailable")]
    Unavailable,
}

pub struct ReinstallCandidatePlanRequest<'a> {
    pub game_id: &'a GameId,
    pub profile_id: &'a ProfileId,
    pub mod_id: &'a ModId,
    pub candidate: &'a StoredModRevision,
    pub layer: &'a FileLayer,
}

pub trait ReinstallCandidatePlanner: Send + Sync {
    fn build_candidate_plan(
        &self,
        request: ReinstallCandidatePlanRequest<'_>,
    ) -> Result<InstallPlan, ReinstallCandidatePlanError>;
}

pub trait ReinstallCandidateSourceReader: Send + Sync {
    fn read_candidate_source_file(
        &self,
        candidate: &StoredModRevision,
        package_file_id: &PackageFileId,
    ) -> anyhow::Result<Vec<u8>>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReinstallPreviewError {
    #[error("reinstall catalog is unavailable")]
    CatalogUnavailable,
    #[error("reinstall manifest is unavailable")]
    ManifestUnavailable,
    #[error("reinstall recovery state is unavailable")]
    RecoveryUnavailable,
    #[error("candidate install plan is unavailable")]
    CandidatePlanUnavailable,
}

#[derive(Clone)]
pub struct ReinstallPreviewService {
    catalog: Arc<dyn ModImportResultRepository>,
    planner: Arc<dyn ReinstallCandidatePlanner>,
    source: Arc<dyn ReinstallCandidateSourceReader>,
    game: Arc<dyn InstallGameFileSystem>,
    backups: Arc<dyn InstallBackupStore>,
    manifests: Arc<dyn InstallManifestRepository>,
    recovery: Arc<dyn ReinstallRecoveryTransactionRepository>,
}

impl ReinstallPreviewService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        catalog: Arc<dyn ModImportResultRepository>,
        planner: Arc<dyn ReinstallCandidatePlanner>,
        source: Arc<dyn ReinstallCandidateSourceReader>,
        game: Arc<dyn InstallGameFileSystem>,
        backups: Arc<dyn InstallBackupStore>,
        manifests: Arc<dyn InstallManifestRepository>,
        recovery: Arc<dyn ReinstallRecoveryTransactionRepository>,
    ) -> Self {
        Self {
            catalog,
            planner,
            source,
            game,
            backups,
            manifests,
            recovery,
        }
    }

    pub fn preview(
        &self,
        request: ReinstallPreviewRequest,
    ) -> Result<ReinstallPlanPreview, ReinstallPreviewError> {
        let candidate = self
            .catalog
            .get_revision(&request.candidate_revision_id)
            .map_err(|_| ReinstallPreviewError::CatalogUnavailable)?;
        let Some(candidate) = candidate else {
            return Ok(ReinstallPlanPreview::blocked(
                None,
                None,
                ReinstallBlockingReason::CandidateNotFound,
            ));
        };
        if candidate.revision_id != request.candidate_revision_id {
            return Err(ReinstallPreviewError::CatalogUnavailable);
        }
        if candidate.mod_id != request.mod_id {
            return Ok(ReinstallPlanPreview::blocked(
                None,
                Some(candidate.revision_id),
                ReinstallBlockingReason::CandidateOwnerMismatch,
            ));
        }
        let candidate_revision_id = candidate.revision_id.clone();

        let logical_mod = self
            .catalog
            .get_mod(&request.mod_id)
            .map_err(|_| ReinstallPreviewError::CatalogUnavailable)?;
        let manifest = self
            .manifests
            .load_manifest(&request.profile_id)
            .map_err(|_| ReinstallPreviewError::ManifestUnavailable)?;
        let Some(manifest) = manifest else {
            return Ok(ReinstallPlanPreview::blocked(
                None,
                Some(candidate_revision_id),
                ReinstallBlockingReason::NotInstalled,
            ));
        };

        if manifest.profile_id != request.profile_id {
            return Ok(ReinstallPlanPreview::blocked(
                None,
                Some(candidate_revision_id),
                ReinstallBlockingReason::ManifestStateUnsafe,
            ));
        }
        if let Err(error) = manifest.validate() {
            let reason = match error {
                InstallManifestValidationError::UnsupportedSchemaVersion { .. } => {
                    ReinstallBlockingReason::ManifestStateUnsafe
                }
                InstallManifestValidationError::RevisionedEntriesRequireSchemaV2
                | InstallManifestValidationError::MixedRevisionSet { .. }
                | InstallManifestValidationError::MultipleRevisionSet { .. } => {
                    ReinstallBlockingReason::InstalledRevisionUnknown
                }
            };
            return Ok(ReinstallPlanPreview::blocked(
                None,
                Some(candidate_revision_id),
                reason,
            ));
        }

        let active_recovery = self
            .recovery
            .load_transaction(&request.profile_id, &request.mod_id)
            .map_err(|_| ReinstallPreviewError::RecoveryUnavailable)?;
        if active_recovery.is_some()
            || manifest.status.consumption() != InstallManifestStatusConsumption::TrustEntries
        {
            return Ok(ReinstallPlanPreview::blocked(
                None,
                Some(candidate_revision_id),
                ReinstallBlockingReason::ManifestStateUnsafe,
            ));
        }

        let legacy_provenance = logical_mod
            .as_ref()
            .filter(|logical_mod| logical_mod.mod_id == request.mod_id)
            .map(|logical_mod| vec![logical_mod.origin_revision_id.clone()])
            .unwrap_or_default();
        let installed_revision_id =
            match resolve_installed_revision(&manifest, &request.mod_id, &legacy_provenance) {
                Ok(revision_id) => revision_id,
                Err(ReinstallManifestError::ModNotInstalled) => {
                    return Ok(ReinstallPlanPreview::blocked(
                        None,
                        Some(candidate_revision_id),
                        ReinstallBlockingReason::NotInstalled,
                    ));
                }
                Err(_) => {
                    return Ok(ReinstallPlanPreview::blocked(
                        None,
                        Some(candidate_revision_id),
                        ReinstallBlockingReason::InstalledRevisionUnknown,
                    ));
                }
            };
        let installed_revision = self
            .catalog
            .get_revision(&installed_revision_id)
            .map_err(|_| ReinstallPreviewError::CatalogUnavailable)?;
        if installed_revision
            .as_ref()
            .is_none_or(|revision| revision.mod_id != request.mod_id)
        {
            return Ok(ReinstallPlanPreview::blocked(
                None,
                Some(candidate_revision_id),
                ReinstallBlockingReason::InstalledRevisionUnknown,
            ));
        }
        if installed_revision_id == candidate_revision_id {
            return Ok(ReinstallPlanPreview::blocked(
                Some(installed_revision_id),
                Some(candidate_revision_id),
                ReinstallBlockingReason::CandidateAlreadyInstalled,
            ));
        }

        let plan = match self
            .planner
            .build_candidate_plan(ReinstallCandidatePlanRequest {
                game_id: &request.game_id,
                profile_id: &request.profile_id,
                mod_id: &request.mod_id,
                candidate: &candidate,
                layer: &request.layer,
            }) {
            Ok(plan) => plan,
            Err(ReinstallCandidatePlanError::NotReady) => {
                return Ok(ReinstallPlanPreview::blocked(
                    Some(installed_revision_id),
                    Some(candidate_revision_id),
                    ReinstallBlockingReason::CandidateNotReady,
                ));
            }
            Err(ReinstallCandidatePlanError::Unavailable) => {
                return Err(ReinstallPreviewError::CandidatePlanUnavailable);
            }
        };

        let candidate_summary = Some(candidate_revision_id.clone());
        let installed_summary = Some(installed_revision_id.clone());
        if plan.has_blocking_conflicts() {
            return Ok(ReinstallPlanPreview::blocked(
                installed_summary,
                candidate_summary,
                ReinstallBlockingReason::PlanConflict,
            ));
        }
        if plan.actions.is_empty() {
            return Ok(ReinstallPlanPreview::blocked(
                installed_summary,
                candidate_summary,
                ReinstallBlockingReason::CandidateNotReady,
            ));
        }
        if plan.actions.iter().any(|action| {
            action.provider.mod_id != request.mod_id
                || action.provider.target_path != action.target_path
        }) {
            return Ok(ReinstallPlanPreview::blocked(
                installed_summary,
                candidate_summary,
                ReinstallBlockingReason::CrossModTargetConflict,
            ));
        }

        let protected_targets = manifest
            .entries
            .iter()
            .filter(|entry| entry.mod_id == request.mod_id)
            .map(|entry| entry.target_path.clone())
            .chain(plan.actions.iter().map(|action| action.target_path.clone()))
            .collect::<BTreeSet<_>>();
        if manifest.entries.iter().any(|entry| {
            entry.mod_id != request.mod_id && protected_targets.contains(&entry.target_path)
        }) {
            return Ok(ReinstallPlanPreview::blocked(
                installed_summary,
                candidate_summary,
                ReinstallBlockingReason::CrossModTargetConflict,
            ));
        }

        let (candidate_states, source_facts) = match self.preload_candidate(&candidate, &plan) {
            Ok(result) => result,
            Err(reason) => {
                return Ok(ReinstallPlanPreview::blocked(
                    installed_summary,
                    candidate_summary,
                    reason,
                ));
            }
        };
        let (installed_states, target_facts, backup_facts) = match self.preflight_installed(
            &request.mod_id,
            &installed_revision_id,
            &manifest,
            &candidate_states,
        ) {
            Ok(result) => result,
            Err(reason) => {
                return Ok(ReinstallPlanPreview::blocked(
                    installed_summary,
                    candidate_summary,
                    reason,
                ));
            }
        };

        let classifications =
            match classify_reinstall_targets(&request.mod_id, installed_states, candidate_states) {
                Ok(classifications) => classifications,
                Err(error) => {
                    let reason = match error {
                        ReinstallClassificationError::CrossModTargetOwnership { .. } => {
                            ReinstallBlockingReason::CrossModTargetConflict
                        }
                        _ => ReinstallBlockingReason::PlanConflict,
                    };
                    return Ok(ReinstallPlanPreview::blocked(
                        installed_summary,
                        candidate_summary,
                        reason,
                    ));
                }
            };

        let mut counts = ReinstallTargetCounts::default();
        for classification in classifications {
            match classification.class {
                ReinstallTargetClass::Retained => counts.retained += 1,
                ReinstallTargetClass::Replaced => counts.replaced += 1,
                ReinstallTargetClass::Added => counts.added += 1,
                ReinstallTargetClass::Stale => counts.stale += 1,
            }
        }

        let plan_token = canonical_plan_token(
            &request,
            &manifest,
            &installed_revision_id,
            &candidate,
            &source_facts,
            &target_facts,
            &backup_facts,
        );
        Ok(ReinstallPlanPreview {
            status: ReinstallPreviewStatus::Ready,
            installed_revision: Some(revision_summary(installed_revision_id)),
            candidate_revision: Some(revision_summary(candidate_revision_id)),
            counts,
            blocking_reasons: Vec::new(),
            plan_token: Some(plan_token),
        })
    }

    fn preload_candidate(
        &self,
        candidate: &StoredModRevision,
        plan: &InstallPlan,
    ) -> Result<(Vec<ReinstallTargetState>, Vec<SourceFact>), ReinstallBlockingReason> {
        let mut grouped =
            BTreeMap::<InstallTargetPath, Vec<(InstallFileProvider, InstalledFileSummary)>>::new();
        let mut facts = Vec::with_capacity(plan.actions.len());
        for action in &plan.actions {
            let bytes = self
                .source
                .read_candidate_source_file(candidate, &action.provider.package_file_id)
                .map_err(|_| ReinstallBlockingReason::SourceUnavailable)?;
            let summary = summarize(&bytes);
            grouped
                .entry(action.target_path.clone())
                .or_default()
                .push((action.provider.clone(), summary.clone()));
            facts.push(SourceFact {
                target_path: action.target_path.clone(),
                provider: action.provider.clone(),
                summary,
            });
        }

        let states = grouped
            .into_iter()
            .map(|(target_path, providers)| {
                let final_file = providers
                    .iter()
                    .max_by_key(|(provider, _)| provider.layer.priority)
                    .expect("candidate target group is non-empty")
                    .1
                    .clone();
                ReinstallTargetState::new(
                    target_path,
                    candidate.revision_id.clone(),
                    providers
                        .into_iter()
                        .map(|(provider, _)| provider)
                        .collect(),
                    final_file,
                )
            })
            .collect();
        Ok((states, facts))
    }

    fn preflight_installed(
        &self,
        requested_mod_id: &ModId,
        installed_revision_id: &ModRevisionId,
        manifest: &InstallManifest,
        candidate_states: &[ReinstallTargetState],
    ) -> Result<InstalledPreflight, ReinstallBlockingReason> {
        let mut grouped = BTreeMap::<InstallTargetPath, Vec<&InstallManifestEntry>>::new();
        for entry in manifest
            .entries
            .iter()
            .filter(|entry| entry.mod_id == *requested_mod_id)
        {
            grouped
                .entry(entry.target_path.clone())
                .or_default()
                .push(entry);
        }

        let mut states = Vec::with_capacity(grouped.len());
        let mut target_facts = BTreeMap::new();
        let mut backup_refs = BTreeSet::new();
        for (target_path, entries) in grouped {
            let winner = entries
                .iter()
                .max_by_key(|entry| entry.layer.priority)
                .expect("installed target group is non-empty");
            let expected = winner
                .installed_file
                .clone()
                .ok_or(ReinstallBlockingReason::ManifestStateUnsafe)?;
            let bytes = self
                .game
                .read_game_file(&target_path)
                .map_err(|_| ReinstallBlockingReason::TargetReadFailed)?
                .ok_or(ReinstallBlockingReason::TargetMissing)?;
            let current = summarize(&bytes);
            if current != expected {
                return Err(ReinstallBlockingReason::TargetChanged);
            }
            target_facts.insert(target_path.clone(), Some(current));

            let refs = entries
                .iter()
                .filter_map(|entry| entry.backup_ref.as_deref())
                .collect::<BTreeSet<_>>();
            if refs.len() > 1 || refs.iter().any(|reference| reference.trim().is_empty()) {
                return Err(ReinstallBlockingReason::ManifestStateUnsafe);
            }
            backup_refs.extend(refs.into_iter().map(str::to_owned));

            states.push(ReinstallTargetState::new(
                target_path,
                installed_revision_id.clone(),
                entries
                    .into_iter()
                    .map(|entry| InstallFileProvider {
                        mod_id: entry.mod_id.clone(),
                        package_file_id: entry.package_file_id.clone(),
                        target_path: entry.target_path.clone(),
                        layer: entry.layer.clone(),
                    })
                    .collect(),
                expected,
            ));
        }

        for candidate in candidate_states {
            if target_facts.contains_key(&candidate.target_path) {
                continue;
            }
            let bytes = self
                .game
                .read_game_file(&candidate.target_path)
                .map_err(|_| ReinstallBlockingReason::TargetReadFailed)?;
            target_facts.insert(
                candidate.target_path.clone(),
                bytes.as_deref().map(summarize),
            );
        }

        let mut backup_facts = BTreeMap::new();
        for backup_ref in backup_refs {
            let bytes = self
                .backups
                .read_backup(&backup_ref)
                .map_err(|_| ReinstallBlockingReason::BackupReadFailed)?
                .ok_or(ReinstallBlockingReason::BackupMissing)?;
            backup_facts.insert(backup_ref, summarize(&bytes));
        }
        Ok((states, target_facts, backup_facts))
    }
}

type InstalledPreflight = (
    Vec<ReinstallTargetState>,
    BTreeMap<InstallTargetPath, Option<InstalledFileSummary>>,
    BTreeMap<String, InstalledFileSummary>,
);

#[derive(Clone)]
struct SourceFact {
    target_path: InstallTargetPath,
    provider: InstallFileProvider,
    summary: InstalledFileSummary,
}

fn revision_summary(revision_id: ModRevisionId) -> ReinstallRevisionSummary {
    ReinstallRevisionSummary { revision_id }
}

fn summarize(bytes: &[u8]) -> InstalledFileSummary {
    InstalledFileSummary {
        size_bytes: bytes.len() as u64,
        sha256: Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    }
}

fn canonical_plan_token(
    request: &ReinstallPreviewRequest,
    manifest: &InstallManifest,
    installed_revision_id: &ModRevisionId,
    candidate: &StoredModRevision,
    source_facts: &[SourceFact],
    target_facts: &BTreeMap<InstallTargetPath, Option<InstalledFileSummary>>,
    backup_facts: &BTreeMap<String, InstalledFileSummary>,
) -> String {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, "reinstall-preview-v1");
    hash_field(&mut hasher, request.game_id.as_str());
    hash_field(&mut hasher, request.profile_id.as_str());
    hash_field(&mut hasher, request.mod_id.as_str());
    hash_field(&mut hasher, installed_revision_id.as_str());
    hash_field(&mut hasher, candidate.revision_id.as_str());
    hash_field(&mut hasher, candidate.mod_id.as_str());
    hash_field(&mut hasher, &candidate.package_id);
    hash_field(&mut hasher, &request.layer.name);
    hash_i32(&mut hasher, request.layer.priority);
    hash_u64(&mut hasher, manifest.schema_version.into());
    hash_field(&mut hasher, manifest_status_code(manifest));

    let mut entries = manifest.entries.iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left.target_path
            .cmp(&right.target_path)
            .then_with(|| left.layer.priority.cmp(&right.layer.priority))
            .then_with(|| left.layer.name.cmp(&right.layer.name))
            .then_with(|| left.mod_id.cmp(&right.mod_id))
            .then_with(|| left.package_file_id.cmp(&right.package_file_id))
    });
    hash_u64(&mut hasher, entries.len() as u64);
    for entry in entries {
        hash_field(&mut hasher, entry.target_path.as_str());
        hash_field(&mut hasher, entry.mod_id.as_str());
        hash_optional(
            &mut hasher,
            entry.revision_id.as_ref().map(ModRevisionId::as_str),
        );
        hash_field(&mut hasher, entry.package_file_id.as_str());
        hash_field(&mut hasher, &entry.layer.name);
        hash_i32(&mut hasher, entry.layer.priority);
        hash_optional(&mut hasher, entry.backup_ref.as_deref());
        hash_optional_summary(&mut hasher, entry.installed_file.as_ref());
    }

    let mut sources = source_facts.to_vec();
    sources.sort_by(|left, right| {
        left.target_path
            .cmp(&right.target_path)
            .then_with(|| {
                left.provider
                    .layer
                    .priority
                    .cmp(&right.provider.layer.priority)
            })
            .then_with(|| {
                left.provider
                    .package_file_id
                    .cmp(&right.provider.package_file_id)
            })
    });
    hash_u64(&mut hasher, sources.len() as u64);
    for source in sources {
        hash_field(&mut hasher, source.target_path.as_str());
        hash_field(&mut hasher, source.provider.mod_id.as_str());
        hash_field(&mut hasher, source.provider.package_file_id.as_str());
        hash_field(&mut hasher, &source.provider.layer.name);
        hash_i32(&mut hasher, source.provider.layer.priority);
        hash_summary(&mut hasher, &source.summary);
    }

    hash_u64(&mut hasher, target_facts.len() as u64);
    for (target, summary) in target_facts {
        hash_field(&mut hasher, target.as_str());
        hash_optional_summary(&mut hasher, summary.as_ref());
    }
    hash_u64(&mut hasher, backup_facts.len() as u64);
    for (backup_ref, summary) in backup_facts {
        hash_field(&mut hasher, backup_ref);
        hash_summary(&mut hasher, summary);
    }

    format!("reinstall-preview-v1:{:x}", hasher.finalize())
}

fn manifest_status_code(manifest: &InstallManifest) -> &'static str {
    use hmm_core::InstallManifestStatus::{
        Committing, Completed, Planned, RepairRequired, RollbackRequired, RolledBack,
    };
    match manifest.status {
        Planned => "planned",
        Committing => "committing",
        Completed => "completed",
        RollbackRequired => "rollback_required",
        RolledBack => "rolled_back",
        RepairRequired => "repair_required",
    }
}

fn hash_field(hasher: &mut Sha256, value: &str) {
    hash_u64(hasher, value.len() as u64);
    hasher.update(value.as_bytes());
}

fn hash_optional(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            hash_field(hasher, value);
        }
        None => hasher.update([0]),
    }
}

fn hash_optional_summary(hasher: &mut Sha256, summary: Option<&InstalledFileSummary>) {
    match summary {
        Some(summary) => {
            hasher.update([1]);
            hash_summary(hasher, summary);
        }
        None => hasher.update([0]),
    }
}

fn hash_summary(hasher: &mut Sha256, summary: &InstalledFileSummary) {
    hash_u64(hasher, summary.size_bytes);
    hash_field(hasher, &summary.sha256);
}

fn hash_i32(hasher: &mut Sha256, value: i32) {
    hasher.update(value.to_be_bytes());
}

fn hash_u64(hasher: &mut Sha256, value: u64) {
    hasher.update(value.to_be_bytes());
}
