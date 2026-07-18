use super::*;
use hmm_core::{Category, ModMetadataOverlay, PreviewImageRejectionReason};
use hmm_ports::{
    CategoryRepository, ModImportResultRepository, ModMetadataRepository, StoredImportPreviewImage,
    StoredModImportAnalysis, StoredModPackageMetadata,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct FakeModImportResultRepository {
    records: Vec<StoredModImportAnalysis>,
    fail_list: bool,
}

impl ModImportResultRepository for FakeModImportResultRepository {
    fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> anyhow::Result<()> {
        Ok(())
    }

    fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
        if self.fail_list {
            anyhow::bail!("library unavailable");
        }
        Ok(self.records.clone())
    }

    fn get_analysis(&self, mod_id: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
        Ok(self
            .records
            .iter()
            .find(|record| record.mod_id == mod_id)
            .cloned())
    }
}

struct FakeMetadataRepository {
    overlays: Vec<ModMetadataOverlay>,
    fail_list: bool,
}

impl ModMetadataRepository for FakeMetadataRepository {
    fn get(&self, mod_id: &str) -> anyhow::Result<Option<ModMetadataOverlay>> {
        Ok(self
            .overlays
            .iter()
            .find(|overlay| overlay.mod_id.as_str() == mod_id)
            .cloned())
    }

    fn save(&self, _overlay: &ModMetadataOverlay) -> anyhow::Result<()> {
        Ok(())
    }

    fn delete(&self, _mod_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn list_all(&self) -> anyhow::Result<Vec<ModMetadataOverlay>> {
        if self.fail_list {
            anyhow::bail!("metadata unavailable");
        }
        Ok(self.overlays.clone())
    }
}

struct FakeCategoryRepository {
    categories: Vec<Category>,
    pairs: Vec<(String, Category)>,
    fail_get: bool,
}

impl CategoryRepository for FakeCategoryRepository {
    fn get(&self, category_id: &str) -> anyhow::Result<Option<Category>> {
        if self.fail_get {
            anyhow::bail!("category unavailable");
        }
        Ok(self
            .categories
            .iter()
            .find(|category| category.id == category_id)
            .cloned())
    }

    fn save(&self, _category: &Category) -> anyhow::Result<()> {
        Ok(())
    }

    fn delete(&self, _category_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn list_all(&self) -> anyhow::Result<Vec<Category>> {
        Ok(self.categories.clone())
    }

    fn count_mods(&self, category_id: &str) -> anyhow::Result<u32> {
        Ok(self
            .pairs
            .iter()
            .filter(|(_, category)| category.id == category_id)
            .count() as u32)
    }

    fn get_mod_categories(&self, mod_id: &str) -> anyhow::Result<Vec<Category>> {
        Ok(self
            .pairs
            .iter()
            .filter(|(candidate_mod_id, _)| candidate_mod_id == mod_id)
            .map(|(_, category)| category.clone())
            .collect())
    }

    fn set_mod_categories(&self, _mod_id: &str, _category_ids: &[String]) -> anyhow::Result<()> {
        Ok(())
    }

    fn list_mod_category_pairs(&self) -> anyhow::Result<Vec<(String, Category)>> {
        Ok(self.pairs.clone())
    }
}

struct FakeStatusProvider {
    statuses: HashMap<String, InstallManifestStatus>,
    override_summaries: Option<Vec<InstallManifestStatusSummary>>,
    fail: bool,
    calls: Mutex<Vec<Vec<String>>>,
}

impl FakeStatusProvider {
    fn empty() -> Self {
        Self {
            statuses: HashMap::new(),
            override_summaries: None,
            fail: false,
            calls: Mutex::new(Vec::new()),
        }
    }

    fn failing() -> Self {
        Self {
            fail: true,
            ..Self::empty()
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.lock().expect("status calls lock").clone()
    }
}

impl ModLibraryStatusProvider for FakeStatusProvider {
    fn query_statuses(
        &self,
        context: &ModLibraryProfileContext,
        mod_ids: &[ModId],
    ) -> Result<Vec<InstallManifestStatusSummary>, ModLibraryStatusProviderError> {
        self.calls.lock().expect("status calls lock").push(
            mod_ids
                .iter()
                .map(|mod_id| mod_id.as_str().to_owned())
                .collect(),
        );
        if self.fail {
            return Err(ModLibraryStatusProviderError::Unavailable);
        }
        if let Some(summaries) = &self.override_summaries {
            return Ok(summaries.clone());
        }

        Ok(mod_ids
            .iter()
            .map(|mod_id| InstallManifestStatusSummary {
                profile_id: context.profile_id.clone(),
                mod_id: mod_id.clone(),
                status: self
                    .statuses
                    .get(mod_id.as_str())
                    .copied()
                    .unwrap_or(InstallManifestStatus::NotInstalled),
                managed_file_count: usize::from(
                    self.statuses.get(mod_id.as_str()) == Some(&InstallManifestStatus::Installed),
                ),
                backup_count: 0,
            })
            .collect())
    }
}

fn record(mod_id: &str, name: &str) -> StoredModImportAnalysis {
    StoredModImportAnalysis {
        mod_id: mod_id.to_owned(),
        task_id: format!("task-{mod_id}"),
        package_id: format!("package-{mod_id}"),
        display_name: name.to_owned(),
        metadata: StoredModPackageMetadata::default(),
        preview_image: StoredImportPreviewImage::Fallback {
            reason: PreviewImageRejectionReason::Missing,
        },
    }
}

fn category(id: &str, name: &str) -> Category {
    Category {
        id: id.to_owned(),
        name: name.to_owned(),
        color: None,
        sort_order: 0,
        created_at: 1,
    }
}

fn overlay(mod_id: &str, name: Option<&str>, author: Option<&str>) -> ModMetadataOverlay {
    ModMetadataOverlay {
        mod_id: ModId::new(mod_id),
        display_name: name.map(str::to_owned),
        author: author.map(str::to_owned),
        version: None,
        description: None,
        nexus_mod_id: None,
        updated_at: 1,
    }
}

fn profile_context() -> ModLibraryProfileContext {
    ModLibraryProfileContext {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("profile-a"),
    }
}

fn test_service(
    records: Vec<StoredModImportAnalysis>,
    overlays: Vec<ModMetadataOverlay>,
    categories: Vec<Category>,
    pairs: Vec<(String, Category)>,
    status_provider: Arc<dyn ModLibraryStatusProvider>,
) -> ModLibraryQueryService {
    test_service_with_failures(
        records,
        overlays,
        categories,
        pairs,
        false,
        false,
        false,
        status_provider,
    )
}

#[allow(clippy::too_many_arguments)]
fn test_service_with_failures(
    records: Vec<StoredModImportAnalysis>,
    overlays: Vec<ModMetadataOverlay>,
    categories: Vec<Category>,
    pairs: Vec<(String, Category)>,
    fail_library: bool,
    fail_metadata: bool,
    fail_category_get: bool,
    status_provider: Arc<dyn ModLibraryStatusProvider>,
) -> ModLibraryQueryService {
    let result_repository: Arc<dyn ModImportResultRepository> =
        Arc::new(FakeModImportResultRepository {
            records,
            fail_list: fail_library,
        });
    let metadata_repository: Arc<dyn ModMetadataRepository> = Arc::new(FakeMetadataRepository {
        overlays,
        fail_list: fail_metadata,
    });
    let category_repository: Arc<dyn CategoryRepository> = Arc::new(FakeCategoryRepository {
        categories,
        pairs,
        fail_get: fail_category_get,
    });
    let library_service = Arc::new(ModLibraryService::new(
        result_repository,
        metadata_repository,
        category_repository,
    ));
    ModLibraryQueryService::new(library_service, status_provider)
}

fn empty_service() -> ModLibraryQueryService {
    test_service(
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Arc::new(FakeStatusProvider::empty()),
    )
}

fn ids(page: &ModLibraryPage) -> Vec<&str> {
    page.items
        .iter()
        .map(|entry| entry.item.id.as_str())
        .collect()
}

#[test]
fn validates_page_page_size_and_search_length() {
    let service = empty_service();

    assert_eq!(
        service.query(ModLibraryQuery {
            page: 0,
            ..ModLibraryQuery::default()
        }),
        Err(ModLibraryQueryError::PageInvalid)
    );
    assert_eq!(
        service.query(ModLibraryQuery {
            page_size: 25,
            ..ModLibraryQuery::default()
        }),
        Err(ModLibraryQueryError::PageSizeUnsupported)
    );
    for page_size in [12, 24, 48, 96] {
        let page = service
            .query(ModLibraryQuery {
                page_size,
                ..ModLibraryQuery::default()
            })
            .expect("allowlisted page size");
        assert_eq!(page.page_size, page_size);
    }

    service
        .query(ModLibraryQuery {
            search: "界".repeat(MAX_MOD_LIBRARY_SEARCH_CHARS),
            ..ModLibraryQuery::default()
        })
        .expect("128 Unicode scalars are allowed");
    assert_eq!(
        service.query(ModLibraryQuery {
            search: "界".repeat(MAX_MOD_LIBRARY_SEARCH_CHARS + 1),
            ..ModLibraryQuery::default()
        }),
        Err(ModLibraryQueryError::SearchTooLong)
    );
}

#[test]
fn covers_page_boundaries_and_clamps_extreme_pages() {
    for count in [0_usize, 1, 11, 12, 13, 23, 24, 25, 95, 96, 97] {
        let records = (0..count)
            .map(|index| record(&format!("mod-{index:03}"), &format!("Mod {index:03}")))
            .collect();
        let page = test_service(
            records,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Arc::new(FakeStatusProvider::empty()),
        )
        .query(ModLibraryQuery {
            page: u64::MAX,
            page_size: 12,
            ..ModLibraryQuery::default()
        })
        .expect("boundary query");

        let expected_page = if count == 0 {
            1
        } else {
            count.div_ceil(12) as u64
        };
        let expected_items = if count == 0 {
            0
        } else {
            count - (expected_page as usize - 1) * 12
        };
        assert_eq!(page.page, expected_page, "count={count}");
        assert_eq!(page.items.len(), expected_items, "count={count}");
        assert_eq!(page.library_total, count, "count={count}");
        assert_eq!(page.matching_total, count, "count={count}");
    }
}

#[test]
fn returns_only_the_requested_page_after_stable_sorting() {
    let records = (0..25)
        .rev()
        .map(|index| record(&format!("mod-{index:03}"), &format!("Mod {index:03}")))
        .collect();
    let page = test_service(
        records,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Arc::new(FakeStatusProvider::empty()),
    )
    .query(ModLibraryQuery {
        page: 2,
        page_size: 12,
        ..ModLibraryQuery::default()
    })
    .expect("second page");

    assert_eq!(page.page, 2);
    assert_eq!(page.items.len(), 12);
    assert_eq!(ids(&page).first(), Some(&"mod-012"));
    assert_eq!(ids(&page).last(), Some(&"mod-023"));
}

#[test]
fn searches_overlay_name_author_metadata_tags_and_user_category_names() {
    let mut name_record = record("mod-name", "  ÉPÉE   Alpha  ");
    name_record.metadata.author = Some("Import Author".to_owned());
    let mut tag_record = record("mod-tag", "Tag Candidate");
    tag_record.metadata.tags = vec!["Visual   FX".to_owned()];
    let user_category = category("cat-quest", "Quest Tools");
    let service = test_service(
        vec![
            name_record,
            record("mod-author", "Author Candidate"),
            record("mod-overlay-name", "Original Name"),
            tag_record,
            record("mod-category", "Category Candidate"),
        ],
        vec![
            overlay("mod-author", None, Some("Overlay Builder")),
            overlay("mod-overlay-name", Some("Renamed Sword"), None),
        ],
        vec![user_category.clone()],
        vec![("mod-category".to_owned(), user_category)],
        Arc::new(FakeStatusProvider::empty()),
    );

    for (search, expected_id) in [
        (" épée alpha ", "mod-name"),
        ("import author", "mod-name"),
        ("overlay builder", "mod-author"),
        ("renamed sword", "mod-overlay-name"),
        ("visual fx", "mod-tag"),
        ("quest tools", "mod-category"),
    ] {
        let page = service
            .query(ModLibraryQuery {
                search: search.to_owned(),
                page_size: 12,
                ..ModLibraryQuery::default()
            })
            .expect("search query");
        assert_eq!(ids(&page), vec![expected_id], "search={search}");
    }
}

#[test]
fn category_filter_uses_id_even_when_labels_are_equal() {
    let category_a = category("cat-a", "Shared");
    let category_b = category("cat-b", "Shared");
    let empty_category = category("cat-empty", "Empty");
    let service = test_service(
        vec![record("mod-a", "Alpha"), record("mod-b", "Beta")],
        Vec::new(),
        vec![
            category_a.clone(),
            category_b.clone(),
            empty_category.clone(),
        ],
        vec![
            ("mod-a".to_owned(), category_a),
            ("mod-b".to_owned(), category_b),
        ],
        Arc::new(FakeStatusProvider::empty()),
    );

    let page = service
        .query(ModLibraryQuery {
            filter: ModLibraryFilter::Category("cat-b".to_owned()),
            page_size: 12,
            ..ModLibraryQuery::default()
        })
        .expect("category id query");
    assert_eq!(ids(&page), vec!["mod-b"]);

    let empty = service
        .query(ModLibraryQuery {
            filter: ModLibraryFilter::Category(empty_category.id),
            page_size: 12,
            ..ModLibraryQuery::default()
        })
        .expect("existing empty category");
    assert_eq!(empty.library_total, 2);
    assert_eq!(empty.matching_total, 0);
    assert_eq!(empty.page, 1);

    assert_eq!(
        service.query(ModLibraryQuery {
            filter: ModLibraryFilter::Category("missing".to_owned()),
            page_size: 12,
            ..ModLibraryQuery::default()
        }),
        Err(ModLibraryQueryError::CategoryNotFound)
    );
}

#[test]
fn status_is_merged_for_the_full_snapshot_before_filtering() {
    let provider = Arc::new(FakeStatusProvider {
        statuses: HashMap::from([
            ("mod-a".to_owned(), InstallManifestStatus::Installed),
            ("mod-b".to_owned(), InstallManifestStatus::CleanupPending),
            ("mod-c".to_owned(), InstallManifestStatus::NotInstalled),
        ]),
        ..FakeStatusProvider::empty()
    });
    let service = test_service(
        vec![
            record("mod-a", "Alpha"),
            record("mod-b", "Beta"),
            record("mod-c", "Gamma"),
        ],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Arc::clone(&provider) as Arc<dyn ModLibraryStatusProvider>,
    );

    let page = service
        .query(ModLibraryQuery {
            profile_context: Some(profile_context()),
            filter: ModLibraryFilter::Status(InstallManifestStatus::CleanupPending),
            page_size: 12,
            ..ModLibraryQuery::default()
        })
        .expect("status filter");

    assert_eq!(ids(&page), vec!["mod-b"]);
    assert_eq!(
        page.items[0]
            .install_summary
            .as_ref()
            .map(|summary| summary.status),
        Some(InstallManifestStatus::CleanupPending)
    );
    assert_eq!(
        provider.calls(),
        vec![vec![
            "mod-a".to_owned(),
            "mod-b".to_owned(),
            "mod-c".to_owned(),
        ]]
    );
}

#[test]
fn profileless_queries_are_available_but_status_filter_fails_closed() {
    let provider = Arc::new(FakeStatusProvider::failing());
    let service = test_service(
        vec![record("mod-a", "Alpha")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Arc::clone(&provider) as Arc<dyn ModLibraryStatusProvider>,
    );

    let page = service
        .query(ModLibraryQuery {
            page_size: 12,
            ..ModLibraryQuery::default()
        })
        .expect("profileless all query");
    assert_eq!(ids(&page), vec!["mod-a"]);
    assert!(page.items[0].install_summary.is_none());
    assert!(provider.calls().is_empty());

    assert_eq!(
        service.query(ModLibraryQuery {
            filter: ModLibraryFilter::Status(InstallManifestStatus::Installed),
            page_size: 12,
            ..ModLibraryQuery::default()
        }),
        Err(ModLibraryQueryError::ProfileContextRequired)
    );
}

#[test]
fn overlay_is_applied_before_name_sort_and_mod_id_breaks_ties() {
    let page = test_service(
        vec![
            record("mod-b", "Same"),
            record("mod-a", "Same"),
            record("mod-c", "Zulu"),
        ],
        vec![overlay("mod-c", Some("Aardvark"), None)],
        Vec::new(),
        Vec::new(),
        Arc::new(FakeStatusProvider::empty()),
    )
    .query(ModLibraryQuery {
        page_size: 12,
        ..ModLibraryQuery::default()
    })
    .expect("sorted query");

    assert_eq!(ids(&page), vec!["mod-c", "mod-a", "mod-b"]);
    assert_eq!(page.items[0].item.name, "Aardvark");
}

#[test]
fn repository_and_status_failures_return_stable_errors() {
    let library_failure = test_service_with_failures(
        vec![record("mod-a", "Alpha")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
        false,
        false,
        Arc::new(FakeStatusProvider::empty()),
    );
    assert_eq!(
        library_failure.query(ModLibraryQuery {
            page_size: 12,
            ..ModLibraryQuery::default()
        }),
        Err(ModLibraryQueryError::LibraryUnavailable)
    );

    let metadata_failure = test_service_with_failures(
        vec![record("mod-a", "Alpha")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        false,
        true,
        false,
        Arc::new(FakeStatusProvider::empty()),
    );
    assert_eq!(
        metadata_failure.query(ModLibraryQuery {
            page_size: 12,
            ..ModLibraryQuery::default()
        }),
        Err(ModLibraryQueryError::LibraryUnavailable)
    );

    let category_a = category("cat-a", "Category");
    let category_failure = test_service_with_failures(
        vec![record("mod-a", "Alpha")],
        Vec::new(),
        vec![category_a],
        Vec::new(),
        false,
        false,
        true,
        Arc::new(FakeStatusProvider::empty()),
    );
    assert_eq!(
        category_failure.query(ModLibraryQuery {
            filter: ModLibraryFilter::Category("cat-a".to_owned()),
            page_size: 12,
            ..ModLibraryQuery::default()
        }),
        Err(ModLibraryQueryError::LibraryUnavailable)
    );

    let status_failure = test_service(
        vec![record("mod-a", "Alpha")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Arc::new(FakeStatusProvider::failing()),
    );
    assert_eq!(
        status_failure.query(ModLibraryQuery {
            profile_context: Some(profile_context()),
            page_size: 12,
            ..ModLibraryQuery::default()
        }),
        Err(ModLibraryQueryError::StatusUnavailable)
    );
}

#[test]
fn malformed_status_responses_and_duplicate_library_ids_fail_closed() {
    let incomplete_provider = Arc::new(FakeStatusProvider {
        override_summaries: Some(Vec::new()),
        ..FakeStatusProvider::empty()
    });
    let incomplete_status = test_service(
        vec![record("mod-a", "Alpha")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        incomplete_provider,
    );
    assert_eq!(
        incomplete_status.query(ModLibraryQuery {
            profile_context: Some(profile_context()),
            page_size: 12,
            ..ModLibraryQuery::default()
        }),
        Err(ModLibraryQueryError::StatusUnavailable)
    );

    let duplicate_library = test_service(
        vec![record("mod-a", "Alpha"), record("mod-a", "Duplicate")],
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Arc::new(FakeStatusProvider::empty()),
    );
    assert_eq!(
        duplicate_library.query(ModLibraryQuery {
            page_size: 12,
            ..ModLibraryQuery::default()
        }),
        Err(ModLibraryQueryError::LibraryUnavailable)
    );
}
