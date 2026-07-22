use crate::mod_library_dto::ModLibraryPageDto;
use hmm_app::{
    InstallManifestQueryRequest, InstallManifestQueryService, InstallManifestStatus,
    ModLibraryFilter, ModLibraryProfileContext, ModLibraryProjectionFreshnessGuard,
    ModLibraryProjectionRefreshService, ModLibraryQuery, ModLibraryQueryService, ModLibraryService,
    ModLibraryStatusProvider,
};
use hmm_core::{
    Category, FileLayer, InstallManifest, InstallManifestEntry, InstallTargetPath, ModId,
    ModMetadataOverlay, ModRevisionId, PackageFileId, PreviewImageRejectionReason, ProfileId,
};
use hmm_infra::{
    JsonInstallManifestRepository, JsonModImportResultRepository,
    SqliteModLibraryProjectionRepository,
};
use hmm_ports::{
    CategoryRepository, InstallManifestRepository, ModImportResultRepository,
    ModLibraryProjectionQueryRepository, ModLibraryProjectionRepository, ModMetadataRepository,
    StoredImportPreviewImage, StoredLogicalMod, StoredModImportAnalysis, StoredModOriginProvenance,
    StoredModPackageMetadata, StoredModRevision,
};
use serde_json::{json, Value};
use std::hint::black_box;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const RECORD_COUNTS: [usize; 2] = [1_000, 10_000];
const PAGE_SIZE: u32 = 96;
const SQLITE_PROJECTION_10K_P95_BUDGET_NS: u128 = 14_230_000;

#[test]
#[ignore = "explicit release-only performance harness"]
fn mod_library_read_model_baseline() {
    if cfg!(debug_assertions) {
        panic!(
            "run with: cargo test -p hmm-tauri --release mod_library_read_model_baseline -- --ignored --nocapture"
        );
    }

    for record_count in RECORD_COUNTS {
        let report = benchmark_case(record_count);
        println!(
            "HMM_MOD_LIBRARY_READ_MODEL_BENCHMARK={}",
            serde_json::to_string(&report).expect("serialize benchmark report")
        );
    }
}

fn benchmark_case(record_count: usize) -> Value {
    let config = BenchmarkConfig::for_record_count(record_count);
    let fixture = BenchmarkFixture::new(record_count);
    let temp = tempfile::tempdir().expect("benchmark temp directory");

    let catalog_path = temp.path().join("mod-revision-catalog.json");
    let catalog_bytes = serde_json::to_vec(&json!({
        "version": 2,
        "mods": &fixture.logical_mods,
        "revisions": &fixture.revisions,
    }))
    .expect("serialize artificial revision catalog");
    std::fs::write(&catalog_path, &catalog_bytes).expect("write artificial revision catalog");
    let json_repository = JsonModImportResultRepository::new(catalog_path);

    let json_read = measure(config, || {
        json_repository
            .list_analysis()
            .expect("read artificial revision catalog")
            .len()
    });

    let library_service = Arc::new(ModLibraryService::new(
        Arc::new(StaticResultRepository {
            records: fixture.records.clone(),
        }),
        Arc::new(StaticMetadataRepository {
            overlays: fixture.overlays.clone(),
        }),
        Arc::new(StaticCategoryRepository {
            categories: fixture.categories.clone(),
            pairs: fixture.category_pairs.clone(),
        }),
    ));
    let snapshot_merge = measure(config, || {
        library_service
            .get_mod_library()
            .expect("merge artificial library snapshot")
            .len()
    });

    let manifest_repository = Arc::new(JsonInstallManifestRepository::new(
        temp.path().join("install-manifests"),
    ));
    manifest_repository
        .save_manifest(&fixture.manifest)
        .expect("write artificial install manifest");
    let status_service = Arc::new(InstallManifestQueryService::new(manifest_repository));
    let status_provider: Arc<dyn ModLibraryStatusProvider> = status_service.clone();
    let profile_id = fixture.manifest.profile_id.clone();
    let mod_ids = fixture
        .records
        .iter()
        .map(|record| ModId::new(&record.mod_id))
        .collect::<Vec<_>>();
    let status_query = measure_with_setup(
        config,
        || InstallManifestQueryRequest {
            profile_id: profile_id.clone(),
            mod_ids: mod_ids.clone(),
        },
        |request| {
            status_service
                .query_statuses(request)
                .expect("query artificial install statuses")
                .len()
        },
    );

    let query_service =
        ModLibraryQueryService::new(Arc::clone(&library_service), status_provider.clone());
    let query_without_status = ModLibraryQuery {
        search: "mod".to_owned(),
        page: 3,
        page_size: PAGE_SIZE,
        ..ModLibraryQuery::default()
    };
    let query_without_status_total = measure_with_setup(
        config,
        || query_without_status.clone(),
        |query| {
            let page = query_service
                .query(query)
                .expect("query artificial library without profile status");
            page.items.len() + page.matching_total
        },
    );

    let profile_query = ModLibraryQuery {
        profile_context: Some(ModLibraryProfileContext {
            game_id: hmm_core::GameId::mhw(),
            profile_id,
        }),
        filter: ModLibraryFilter::Status(InstallManifestStatus::Installed),
        page: 3,
        page_size: PAGE_SIZE,
        ..ModLibraryQuery::default()
    };
    let profile_query_total = measure_with_setup(
        config,
        || profile_query.clone(),
        |query| {
            let page = query_service
                .query(query)
                .expect("query artificial library with profile status");
            page.items.len() + page.matching_total
        },
    );

    let projection_db = Arc::new(Mutex::new(
        hmm_infra::open_database(&temp.path().join("projection.db"))
            .expect("open artificial projection database"),
    ));
    let projection_repository = Arc::new(SqliteModLibraryProjectionRepository::new(projection_db));
    let projection_writer: Arc<dyn ModLibraryProjectionRepository> = projection_repository.clone();
    let projection_query_repository: Arc<dyn ModLibraryProjectionQueryRepository> =
        projection_repository;
    let projection_query_service = ModLibraryQueryService::new_projection(
        projection_query_repository,
        Arc::new(ModLibraryProjectionRefreshService::new(
            Arc::clone(&library_service),
            status_provider,
            projection_writer,
            Arc::new(ModLibraryProjectionFreshnessGuard::default()),
        )),
    );
    let projected_page = projection_query_service
        .query(profile_query.clone())
        .expect("warm artificial SQLite projection query");
    let compatible_page = query_service
        .query(profile_query.clone())
        .expect("prepare compatibility comparison page");
    assert_eq!(projected_page, compatible_page);
    let sqlite_projection_status_filter_query_total = measure_with_setup(
        config,
        || profile_query.clone(),
        |query| {
            let page = projection_query_service
                .query(query)
                .expect("query artificial SQLite projection with profile status");
            page.items.len() + page.matching_total
        },
    );
    if record_count == 10_000 {
        assert!(
            sqlite_projection_status_filter_query_total.p95_ns
                <= SQLITE_PROJECTION_10K_P95_BUDGET_NS,
            "10,000-record SQLite projection status-filter query p95 {}ns exceeds {}ns same-machine budget",
            sqlite_projection_status_filter_query_total.p95_ns,
            SQLITE_PROJECTION_10K_P95_BUDGET_NS
        );
    }

    let serialization_page = query_service
        .query(query_without_status.clone())
        .expect("prepare artificial serialization page");
    assert_eq!(serialization_page.items.len(), PAGE_SIZE as usize);
    let dto_serialization = measure_with_setup(
        config,
        || serialization_page.clone(),
        |page| {
            let dto = ModLibraryPageDto::from(page);
            serde_json::to_vec(&dto)
                .expect("serialize artificial page DTO")
                .len()
        },
    );

    json!({
        "schemaVersion": 1,
        "profile": "release",
        "recordCount": record_count,
        "installedManifestEntryCount": fixture.manifest.entries.len(),
        "catalogBytes": catalog_bytes.len(),
        "warmupIterations": config.warmup_iterations,
        "sampleIterations": config.sample_iterations,
        "metrics": {
            "jsonCatalogReadProject": json_read.as_json(),
            "snapshotOverlayCategoryMerge": snapshot_merge.as_json(),
            "profileStatusQuery": status_query.as_json(),
            "queryWithoutStatusTotal": query_without_status_total.as_json(),
            "profileStatusFilterQueryTotal": profile_query_total.as_json(),
            "sqliteProjectionStatusFilterQueryTotal": sqlite_projection_status_filter_query_total.as_json(),
            "pageDtoSerialization": dto_serialization.as_json(),
        }
    })
}

#[derive(Clone, Copy)]
struct BenchmarkConfig {
    warmup_iterations: usize,
    sample_iterations: usize,
}

impl BenchmarkConfig {
    fn for_record_count(record_count: usize) -> Self {
        match record_count {
            1_000 | 10_000 => Self {
                warmup_iterations: 5,
                sample_iterations: 40,
            },
            _ => unreachable!("record count is fixed by the benchmark contract"),
        }
    }
}

struct Measurement {
    median_ns: u128,
    p95_ns: u128,
    min_ns: u128,
    max_ns: u128,
    output: usize,
}

impl Measurement {
    fn as_json(&self) -> Value {
        json!({
            "medianNs": self.median_ns,
            "p95Ns": self.p95_ns,
            "minNs": self.min_ns,
            "maxNs": self.max_ns,
            "output": self.output,
        })
    }
}

fn measure(config: BenchmarkConfig, mut action: impl FnMut() -> usize) -> Measurement {
    measure_with_setup(config, || (), |_| action())
}

fn measure_with_setup<T>(
    config: BenchmarkConfig,
    mut setup: impl FnMut() -> T,
    mut action: impl FnMut(T) -> usize,
) -> Measurement {
    for _ in 0..config.warmup_iterations {
        let input = setup();
        black_box(action(input));
    }

    let mut samples = Vec::with_capacity(config.sample_iterations);
    let mut output = 0;
    for _ in 0..config.sample_iterations {
        let input = setup();
        let started = Instant::now();
        output = black_box(action(input));
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    let p95_index = (samples.len() * 95).div_ceil(100).saturating_sub(1);

    Measurement {
        median_ns: samples[samples.len() / 2],
        p95_ns: samples[p95_index],
        min_ns: samples[0],
        max_ns: samples[samples.len() - 1],
        output,
    }
}

struct BenchmarkFixture {
    records: Vec<StoredModImportAnalysis>,
    logical_mods: Vec<StoredLogicalMod>,
    revisions: Vec<StoredModRevision>,
    overlays: Vec<ModMetadataOverlay>,
    categories: Vec<Category>,
    category_pairs: Vec<(String, Category)>,
    manifest: InstallManifest,
}

impl BenchmarkFixture {
    fn new(record_count: usize) -> Self {
        let categories = (0..32)
            .map(|index| Category {
                id: format!("category-{index:02}"),
                name: format!("Category {index:02}"),
                color: (index % 2 == 0).then(|| format!("#{index:02X}6A8F")),
                sort_order: index,
                created_at: 1,
            })
            .collect::<Vec<_>>();
        let mut records = Vec::with_capacity(record_count);
        let mut logical_mods = Vec::with_capacity(record_count);
        let mut revisions = Vec::with_capacity(record_count);
        let mut overlays = Vec::with_capacity(record_count / 3);
        let mut category_pairs = Vec::with_capacity(record_count + record_count / 10);
        let mut manifest_entries = Vec::with_capacity(record_count / 4);

        for index in 0..record_count {
            let mod_id = format!("mod-{index:05}");
            let revision_id = ModRevisionId::new(format!("revision-{index:05}"));
            let metadata = StoredModPackageMetadata {
                version: Some(format!("{}.{}", index % 8, index % 17)),
                author: Some(format!("Author {:02}", index % 64)),
                category: Some(format!("Imported {:02}", index % 12)),
                tags: vec![
                    format!("Tag {:02}", index % 16),
                    format!("Series {:02}", index % 7),
                ],
                dependencies: Vec::new(),
            };
            let record = StoredModImportAnalysis {
                mod_id: mod_id.clone(),
                task_id: format!("task-{index:05}"),
                package_id: revision_id.as_str().to_owned(),
                display_name: format!("Mod {:05} Variant {:02}", record_count - index, index % 23),
                metadata: metadata.clone(),
                preview_image: StoredImportPreviewImage::Fallback {
                    reason: PreviewImageRejectionReason::Missing,
                },
            };
            logical_mods.push(StoredLogicalMod {
                mod_id: ModId::new(&mod_id),
                origin_revision_id: revision_id.clone(),
                display_revision_id: revision_id.clone(),
                origin_provenance: StoredModOriginProvenance::Imported,
            });
            revisions.push(StoredModRevision {
                revision_id,
                mod_id: ModId::new(&mod_id),
                import_task_id: record.task_id.clone(),
                package_id: record.package_id.clone(),
                display_name: record.display_name.clone(),
                metadata,
                preview_image: record.preview_image.clone(),
            });
            records.push(record);

            if index % 3 == 0 {
                overlays.push(ModMetadataOverlay {
                    mod_id: ModId::new(&mod_id),
                    display_name: Some(format!("Overlay {:05}", record_count - index)),
                    author: Some(format!("Overlay Author {:02}", index % 32)),
                    version: Some(format!("{}.{}", index % 5, index % 11)),
                    description: None,
                    nexus_mod_id: None,
                    updated_at: 1,
                });
            }

            category_pairs.push((mod_id.clone(), categories[index % categories.len()].clone()));
            if index % 10 == 0 {
                category_pairs.push((
                    mod_id.clone(),
                    categories[(index + 7) % categories.len()].clone(),
                ));
            }

            if index % 4 == 0 {
                let target = format!("nativePC/benchmark/{mod_id}.bin");
                manifest_entries.push(InstallManifestEntry {
                    target_path: InstallTargetPath::parse(&target, ["nativePC"])
                        .expect("artificial target path"),
                    mod_id: ModId::new(&mod_id),
                    revision_id: None,
                    package_file_id: PackageFileId::new(&target),
                    layer: FileLayer::new("base", 0),
                    backup_ref: (index % 8 == 0).then(|| format!("backup-{index:05}")),
                    installed_file: None,
                });
            }
        }

        Self {
            records,
            logical_mods,
            revisions,
            overlays,
            categories,
            category_pairs,
            manifest: InstallManifest::completed(
                ProfileId::new("benchmark-profile"),
                manifest_entries,
            ),
        }
    }
}

struct StaticResultRepository {
    records: Vec<StoredModImportAnalysis>,
}

impl ModImportResultRepository for StaticResultRepository {
    fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> anyhow::Result<()> {
        anyhow::bail!("benchmark repository is read-only")
    }

    fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
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

struct StaticMetadataRepository {
    overlays: Vec<ModMetadataOverlay>,
}

impl ModMetadataRepository for StaticMetadataRepository {
    fn get(&self, mod_id: &str) -> anyhow::Result<Option<ModMetadataOverlay>> {
        Ok(self
            .overlays
            .iter()
            .find(|overlay| overlay.mod_id.as_str() == mod_id)
            .cloned())
    }

    fn save(&self, _overlay: &ModMetadataOverlay) -> anyhow::Result<()> {
        anyhow::bail!("benchmark repository is read-only")
    }

    fn delete(&self, _mod_id: &str) -> anyhow::Result<()> {
        anyhow::bail!("benchmark repository is read-only")
    }

    fn list_all(&self) -> anyhow::Result<Vec<ModMetadataOverlay>> {
        Ok(self.overlays.clone())
    }
}

struct StaticCategoryRepository {
    categories: Vec<Category>,
    pairs: Vec<(String, Category)>,
}

impl CategoryRepository for StaticCategoryRepository {
    fn get(&self, category_id: &str) -> anyhow::Result<Option<Category>> {
        Ok(self
            .categories
            .iter()
            .find(|category| category.id == category_id)
            .cloned())
    }

    fn save(&self, _category: &Category) -> anyhow::Result<()> {
        anyhow::bail!("benchmark repository is read-only")
    }

    fn delete(&self, _category_id: &str) -> anyhow::Result<()> {
        anyhow::bail!("benchmark repository is read-only")
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
            .filter(|(candidate, _)| candidate == mod_id)
            .map(|(_, category)| category.clone())
            .collect())
    }

    fn set_mod_categories(&self, _mod_id: &str, _category_ids: &[String]) -> anyhow::Result<()> {
        anyhow::bail!("benchmark repository is read-only")
    }

    fn list_mod_category_pairs(&self) -> anyhow::Result<Vec<(String, Category)>> {
        Ok(self.pairs.clone())
    }
}
