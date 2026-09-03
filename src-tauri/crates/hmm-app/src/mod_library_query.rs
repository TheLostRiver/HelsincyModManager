use crate::mod_library_projection::{
    map_projection_query_error, projection_query_request, ModLibraryProjectionRefreshService,
};
use crate::{
    InstallManifestQueryRequest, InstallManifestQueryService, InstallManifestStatus,
    InstallManifestStatusSummary, ModLibraryItem, ModLibraryService,
};
use hmm_core::{GameId, ModId, ProfileId};
use hmm_ports::{
    normalize_mod_library_query_key, ModLibraryProjectionPageItem,
    ModLibraryProjectionQueryRepository, ModLibraryProjectionStatus,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;

pub const DEFAULT_MOD_LIBRARY_PAGE_SIZE: u32 = 24;
pub const MAX_MOD_LIBRARY_SEARCH_CHARS: usize = 128;
const ALLOWED_PAGE_SIZES: [u32; 4] = [12, 24, 48, 96];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryProfileContext {
    pub game_id: GameId,
    pub profile_id: ProfileId,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ModLibraryFilter {
    #[default]
    All,
    Status(InstallManifestStatus),
    Category(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModLibrarySort {
    #[default]
    NameAsc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryQuery {
    pub profile_context: Option<ModLibraryProfileContext>,
    pub search: String,
    pub filter: ModLibraryFilter,
    pub sort: ModLibrarySort,
    pub page: u64,
    pub page_size: u32,
}

impl Default for ModLibraryQuery {
    fn default() -> Self {
        Self {
            profile_context: None,
            search: String::new(),
            filter: ModLibraryFilter::All,
            sort: ModLibrarySort::NameAsc,
            page: 1,
            page_size: DEFAULT_MOD_LIBRARY_PAGE_SIZE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryPageItem {
    pub item: ModLibraryItem,
    pub install_summary: Option<InstallManifestStatusSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModLibraryPage {
    pub items: Vec<ModLibraryPageItem>,
    pub page: u64,
    pub page_size: u32,
    pub library_total: usize,
    pub matching_total: usize,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModLibraryStatusProviderError {
    #[error("mod library install status is unavailable")]
    Unavailable,
}

pub trait ModLibraryStatusProvider: Send + Sync {
    fn query_statuses(
        &self,
        context: &ModLibraryProfileContext,
        mod_ids: &[ModId],
    ) -> Result<Vec<InstallManifestStatusSummary>, ModLibraryStatusProviderError>;
}

impl ModLibraryStatusProvider for InstallManifestQueryService {
    fn query_statuses(
        &self,
        context: &ModLibraryProfileContext,
        mod_ids: &[ModId],
    ) -> Result<Vec<InstallManifestStatusSummary>, ModLibraryStatusProviderError> {
        InstallManifestQueryService::query_statuses(
            self,
            InstallManifestQueryRequest {
                profile_id: context.profile_id.clone(),
                mod_ids: mod_ids.to_vec(),
            },
        )
        .map_err(|_| ModLibraryStatusProviderError::Unavailable)
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModLibraryQueryError {
    #[error("mod library page must start at one")]
    PageInvalid,
    #[error("mod library page size is unsupported")]
    PageSizeUnsupported,
    #[error("mod library search is too long")]
    SearchTooLong,
    #[error("mod library category was not found")]
    CategoryNotFound,
    #[error("mod library profile context is required for status filtering")]
    ProfileContextRequired,
    #[error("mod library data is unavailable")]
    LibraryUnavailable,
    #[error("mod library install status is unavailable")]
    StatusUnavailable,
}

pub struct ModLibraryQueryService {
    backend: ModLibraryQueryBackend,
}

enum ModLibraryQueryBackend {
    Compatibility {
        library_service: Arc<ModLibraryService>,
        status_provider: Arc<dyn ModLibraryStatusProvider>,
    },
    Projection {
        query_repository: Arc<dyn ModLibraryProjectionQueryRepository>,
        refresh_service: Arc<ModLibraryProjectionRefreshService>,
    },
}

impl ModLibraryQueryService {
    pub fn new(
        library_service: Arc<ModLibraryService>,
        status_provider: Arc<dyn ModLibraryStatusProvider>,
    ) -> Self {
        Self {
            backend: ModLibraryQueryBackend::Compatibility {
                library_service,
                status_provider,
            },
        }
    }

    pub fn new_projection(
        query_repository: Arc<dyn ModLibraryProjectionQueryRepository>,
        refresh_service: Arc<ModLibraryProjectionRefreshService>,
    ) -> Self {
        Self {
            backend: ModLibraryQueryBackend::Projection {
                query_repository,
                refresh_service,
            },
        }
    }

    pub fn query(&self, query: ModLibraryQuery) -> Result<ModLibraryPage, ModLibraryQueryError> {
        validate_query(&query)?;
        match &self.backend {
            ModLibraryQueryBackend::Compatibility {
                library_service,
                status_provider,
            } => Self::query_compatibility(&query, library_service, status_provider),
            ModLibraryQueryBackend::Projection {
                query_repository,
                refresh_service,
            } => Self::query_projection(&query, query_repository, refresh_service),
        }
    }

    fn query_compatibility(
        query: &ModLibraryQuery,
        library_service: &Arc<ModLibraryService>,
        status_provider: &Arc<dyn ModLibraryStatusProvider>,
    ) -> Result<ModLibraryPage, ModLibraryQueryError> {
        let snapshot = library_service
            .get_mod_library_snapshot()
            .map_err(|_| ModLibraryQueryError::LibraryUnavailable)?;
        let library_total = snapshot.len();

        if let ModLibraryFilter::Category(category_id) = &query.filter {
            let exists = library_service
                .category_exists(category_id)
                .map_err(|_| ModLibraryQueryError::LibraryUnavailable)?;
            if !exists {
                return Err(ModLibraryQueryError::CategoryNotFound);
            }
        }

        let mod_ids = snapshot
            .iter()
            .map(|entry| ModId::new(&entry.item.id))
            .collect::<Vec<_>>();
        ensure_unique_mod_ids(&mod_ids)?;
        let mut status_by_mod_id = load_statuses(status_provider, query, &mod_ids)?;
        let normalized_search = normalize_text(&query.search);

        let mut candidates = snapshot
            .into_iter()
            .map(|entry| {
                let install_summary = status_by_mod_id.remove(&entry.item.id);
                let normalized_name = normalize_text(&entry.item.name);
                ModLibraryCandidate {
                    entry,
                    install_summary,
                    normalized_name,
                }
            })
            .filter(|candidate| matches_search(candidate, &normalized_search))
            .filter(|candidate| matches_filter(candidate, &query.filter))
            .collect::<Vec<_>>();

        match query.sort {
            ModLibrarySort::NameAsc => candidates.sort_by(|left, right| {
                left.normalized_name
                    .cmp(&right.normalized_name)
                    .then_with(|| left.entry.item.id.cmp(&right.entry.item.id))
            }),
        }

        let matching_total = candidates.len();
        let page = clamped_page(query.page, query.page_size, matching_total);
        let start = page_start(page, query.page_size, matching_total);
        let items = candidates
            .into_iter()
            .skip(start)
            .take(query.page_size as usize)
            .map(|candidate| ModLibraryPageItem {
                item: candidate.entry.item,
                install_summary: candidate.install_summary,
            })
            .collect();

        Ok(ModLibraryPage {
            items,
            page,
            page_size: query.page_size,
            library_total,
            matching_total,
        })
    }

    fn query_projection(
        query: &ModLibraryQuery,
        query_repository: &Arc<dyn ModLibraryProjectionQueryRepository>,
        refresh_service: &Arc<ModLibraryProjectionRefreshService>,
    ) -> Result<ModLibraryPage, ModLibraryQueryError> {
        let (state, profile_fingerprint) =
            refresh_service.ensure(query.profile_context.as_ref())?;
        let request = projection_query_request(query, &state, profile_fingerprint)?;
        let page = query_repository
            .query(&request)
            .map_err(map_projection_query_error)?;
        let items = page
            .items
            .into_iter()
            .map(|item| projection_page_item_to_app(item, query.profile_context.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ModLibraryPage {
            items,
            page: page.page,
            page_size: page.page_size,
            library_total: page.library_total,
            matching_total: page.matching_total,
        })
    }
}

fn load_statuses(
    status_provider: &Arc<dyn ModLibraryStatusProvider>,
    query: &ModLibraryQuery,
    mod_ids: &[ModId],
) -> Result<HashMap<String, InstallManifestStatusSummary>, ModLibraryQueryError> {
    let Some(context) = &query.profile_context else {
        return Ok(HashMap::new());
    };
    if mod_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let summaries = status_provider
        .query_statuses(context, mod_ids)
        .map_err(|_| ModLibraryQueryError::StatusUnavailable)?;
    validated_status_map(context, mod_ids, summaries)
}

fn projection_page_item_to_app(
    page_item: ModLibraryProjectionPageItem,
    context: Option<&ModLibraryProfileContext>,
) -> Result<ModLibraryPageItem, ModLibraryQueryError> {
    let record = page_item.record;
    let install_summary = match context {
        Some(context) => {
            let (status, managed_file_count, backup_count) = match page_item.status {
                Some(status) => {
                    if status.mod_id != record.mod_id {
                        return Err(ModLibraryQueryError::StatusUnavailable);
                    }
                    (
                        install_status_from_projection(status.status),
                        usize::try_from(status.managed_file_count)
                            .map_err(|_| ModLibraryQueryError::StatusUnavailable)?,
                        usize::try_from(status.backup_count)
                            .map_err(|_| ModLibraryQueryError::StatusUnavailable)?,
                    )
                }
                None => (InstallManifestStatus::NotInstalled, 0, 0),
            };
            Some(InstallManifestStatusSummary {
                profile_id: context.profile_id.clone(),
                mod_id: record.mod_id.clone(),
                status,
                managed_file_count,
                backup_count,
                // The library projection does not carry revisioned manifest facts; batch
                // flows read revisions through `get_install_manifest_status` instead.
                installed_revision_id: None,
                // Same for adopted entries: the uninstall confirmation reads the count
                // through `get_install_manifest_status`, never from the projection.
                adopted_file_count: None,
            })
        }
        None if page_item.status.is_none() => None,
        None => return Err(ModLibraryQueryError::LibraryUnavailable),
    };
    Ok(ModLibraryPageItem {
        item: ModLibraryItem {
            id: record.mod_id.as_str().to_owned(),
            name: record.display_name,
            author: record.author,
            version_label: record.version_label,
            status: crate::ModLibraryStatus::Disabled,
            size_label: record.size_label,
            category_labels: record
                .labels
                .into_iter()
                .map(|label| hmm_core::CategoryLabel {
                    name: label.name,
                    color: label.color,
                })
                .collect(),
            preview_image: crate::mod_import::import_preview_from_stored(record.preview_image),
            external_import_adapter_id: record.external_import_adapter_id,
        },
        install_summary,
    })
}

fn install_status_from_projection(status: ModLibraryProjectionStatus) -> InstallManifestStatus {
    match status {
        ModLibraryProjectionStatus::Installed => InstallManifestStatus::Installed,
        ModLibraryProjectionStatus::CommittedCleanupPending => {
            InstallManifestStatus::CommittedCleanupPending
        }
        ModLibraryProjectionStatus::CleanupPending => InstallManifestStatus::CleanupPending,
        ModLibraryProjectionStatus::RollbackRequired => InstallManifestStatus::RollbackRequired,
        ModLibraryProjectionStatus::RepairRequired => InstallManifestStatus::RepairRequired,
    }
}

struct ModLibraryCandidate {
    entry: crate::mod_import::ModLibrarySnapshotItem,
    install_summary: Option<InstallManifestStatusSummary>,
    normalized_name: String,
}

fn validate_query(query: &ModLibraryQuery) -> Result<(), ModLibraryQueryError> {
    if query.page == 0 {
        return Err(ModLibraryQueryError::PageInvalid);
    }
    if !ALLOWED_PAGE_SIZES.contains(&query.page_size) {
        return Err(ModLibraryQueryError::PageSizeUnsupported);
    }
    if query.search.chars().count() > MAX_MOD_LIBRARY_SEARCH_CHARS {
        return Err(ModLibraryQueryError::SearchTooLong);
    }
    if matches!(query.filter, ModLibraryFilter::Status(_)) && query.profile_context.is_none() {
        return Err(ModLibraryQueryError::ProfileContextRequired);
    }
    Ok(())
}

fn ensure_unique_mod_ids(mod_ids: &[ModId]) -> Result<(), ModLibraryQueryError> {
    let unique = mod_ids.iter().map(ModId::as_str).collect::<HashSet<_>>();
    if unique.len() != mod_ids.len() {
        return Err(ModLibraryQueryError::LibraryUnavailable);
    }
    Ok(())
}

pub(crate) fn validated_status_map(
    context: &ModLibraryProfileContext,
    mod_ids: &[ModId],
    summaries: Vec<InstallManifestStatusSummary>,
) -> Result<HashMap<String, InstallManifestStatusSummary>, ModLibraryQueryError> {
    let requested = mod_ids.iter().map(ModId::as_str).collect::<HashSet<_>>();
    let mut result = HashMap::with_capacity(summaries.len());

    for summary in summaries {
        if summary.profile_id != context.profile_id
            || !requested.contains(summary.mod_id.as_str())
            || result
                .insert(summary.mod_id.as_str().to_owned(), summary)
                .is_some()
        {
            return Err(ModLibraryQueryError::StatusUnavailable);
        }
    }
    if result.len() != requested.len() {
        return Err(ModLibraryQueryError::StatusUnavailable);
    }
    Ok(result)
}

pub(crate) fn normalize_text(value: &str) -> String {
    normalize_mod_library_query_key(value)
}

fn matches_search(candidate: &ModLibraryCandidate, search: &str) -> bool {
    search.is_empty()
        || candidate.normalized_name.contains(search)
        || candidate
            .entry
            .item
            .author
            .as_deref()
            .is_some_and(|author| normalize_text(author).contains(search))
        || candidate
            .entry
            .item
            .category_labels
            .iter()
            .any(|label| normalize_text(&label.name).contains(search))
}

fn matches_filter(candidate: &ModLibraryCandidate, filter: &ModLibraryFilter) -> bool {
    match filter {
        ModLibraryFilter::All => true,
        ModLibraryFilter::Status(status) => candidate
            .install_summary
            .as_ref()
            .is_some_and(|summary| summary.status == *status),
        ModLibraryFilter::Category(category_id) => candidate
            .entry
            .user_category_ids
            .iter()
            .any(|id| id == category_id),
    }
}

fn clamped_page(requested_page: u64, page_size: u32, matching_total: usize) -> u64 {
    if matching_total == 0 {
        return 1;
    }
    let total_pages = matching_total.div_ceil(page_size as usize) as u64;
    requested_page.min(total_pages)
}

fn page_start(page: u64, page_size: u32, matching_total: usize) -> usize {
    if matching_total == 0 {
        return 0;
    }
    ((page - 1) * u64::from(page_size)) as usize
}

#[cfg(test)]
#[path = "mod_library_query_tests.rs"]
mod tests;
