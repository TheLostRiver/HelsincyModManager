use crate::{
    InstallManifestQueryRequest, InstallManifestQueryService, InstallManifestStatus,
    InstallManifestStatusSummary, ModLibraryItem, ModLibraryService,
};
use hmm_core::{GameId, ModId, ProfileId};
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
    library_service: Arc<ModLibraryService>,
    status_provider: Arc<dyn ModLibraryStatusProvider>,
}

impl ModLibraryQueryService {
    pub fn new(
        library_service: Arc<ModLibraryService>,
        status_provider: Arc<dyn ModLibraryStatusProvider>,
    ) -> Self {
        Self {
            library_service,
            status_provider,
        }
    }

    pub fn query(&self, query: ModLibraryQuery) -> Result<ModLibraryPage, ModLibraryQueryError> {
        validate_query(&query)?;

        let snapshot = self
            .library_service
            .get_mod_library_snapshot()
            .map_err(|_| ModLibraryQueryError::LibraryUnavailable)?;
        let library_total = snapshot.len();

        if let ModLibraryFilter::Category(category_id) = &query.filter {
            let exists = self
                .library_service
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
        let mut status_by_mod_id = self.load_statuses(&query, &mod_ids)?;
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

    fn load_statuses(
        &self,
        query: &ModLibraryQuery,
        mod_ids: &[ModId],
    ) -> Result<HashMap<String, InstallManifestStatusSummary>, ModLibraryQueryError> {
        let Some(context) = &query.profile_context else {
            return Ok(HashMap::new());
        };
        if mod_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let summaries = self
            .status_provider
            .query_statuses(context, mod_ids)
            .map_err(|_| ModLibraryQueryError::StatusUnavailable)?;
        validated_status_map(context, mod_ids, summaries)
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

fn validated_status_map(
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

fn normalize_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
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
