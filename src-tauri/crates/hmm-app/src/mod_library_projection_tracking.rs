use anyhow::Result;
use hmm_core::{Category, InstallManifest, ModId, ModMetadataOverlay, ModRevisionId, ProfileId};
use hmm_ports::{
    CategoryRepository, InstallManifestRepository, ModImportCatalogSnapshot,
    ModImportCatalogUpsert, ModImportExternalCatalogUpsert, ModImportResultRepository,
    ModLibraryProjectionRepository, ModMetadataRepository, StoredLogicalMod,
    StoredModImportAnalysis, StoredModRevision,
};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub struct ModLibraryProjectionFreshnessGuard {
    global_unavailable: AtomicBool,
    unavailable_profiles: Mutex<HashSet<String>>,
    // Coordinates an authoritative write's dirty markers with a projection rebuild.
    projection_activity: RwLock<()>,
}

impl ModLibraryProjectionFreshnessGuard {
    pub fn mark_global_unavailable(&self) {
        self.global_unavailable.store(true, Ordering::SeqCst);
    }

    pub(crate) fn global_is_unavailable(&self) -> bool {
        self.global_unavailable.load(Ordering::SeqCst)
    }

    fn clear_global(&self) {
        self.global_unavailable.store(false, Ordering::SeqCst);
    }

    pub fn mark_profile_unavailable(&self, profile_id: &ProfileId) {
        if let Ok(mut profiles) = self.unavailable_profiles.lock() {
            profiles.insert(profile_id.as_str().to_owned());
        }
    }

    pub(crate) fn profile_is_unavailable(&self, profile_id: &ProfileId) -> bool {
        self.unavailable_profiles
            .lock()
            .map(|profiles| profiles.contains(profile_id.as_str()))
            .unwrap_or(true)
    }

    pub(crate) fn clear_profile(&self, profile_id: &ProfileId) {
        if let Ok(mut profiles) = self.unavailable_profiles.lock() {
            profiles.remove(profile_id.as_str());
        }
    }

    pub(crate) fn begin_authoritative_write(&self) -> Result<RwLockReadGuard<'_, ()>> {
        self.projection_activity
            .read()
            .map_err(|_| anyhow::anyhow!("Mod library projection activity lock poisoned"))
    }

    pub(crate) fn begin_refresh(&self) -> Result<RwLockWriteGuard<'_, ()>> {
        self.projection_activity
            .write()
            .map_err(|_| anyhow::anyhow!("Mod library projection activity lock poisoned"))
    }
}

impl Default for ModLibraryProjectionFreshnessGuard {
    fn default() -> Self {
        Self {
            global_unavailable: AtomicBool::new(false),
            unavailable_profiles: Mutex::new(HashSet::new()),
            projection_activity: RwLock::new(()),
        }
    }
}

#[derive(Clone)]
struct GlobalProjectionTracker {
    projection: Arc<dyn ModLibraryProjectionRepository>,
    freshness_guard: Arc<ModLibraryProjectionFreshnessGuard>,
}

impl GlobalProjectionTracker {
    fn new(
        projection: Arc<dyn ModLibraryProjectionRepository>,
        freshness_guard: Arc<ModLibraryProjectionFreshnessGuard>,
    ) -> Self {
        Self {
            projection,
            freshness_guard,
        }
    }

    fn begin_write(&self) -> Result<RwLockReadGuard<'_, ()>> {
        let activity_guard = self.freshness_guard.begin_authoritative_write()?;
        self.projection.mark_dirty(None)?;
        self.freshness_guard.clear_global();
        Ok(activity_guard)
    }

    fn finish_write(&self) {
        if self.projection.mark_dirty(None).is_err() {
            self.freshness_guard.mark_global_unavailable();
        } else {
            self.freshness_guard.clear_global();
        }
    }
}

pub struct ProjectionTrackingModImportResultRepository {
    delegate: Arc<dyn ModImportResultRepository>,
    tracker: GlobalProjectionTracker,
}

impl ProjectionTrackingModImportResultRepository {
    pub fn new(
        delegate: Arc<dyn ModImportResultRepository>,
        projection: Arc<dyn ModLibraryProjectionRepository>,
        freshness_guard: Arc<ModLibraryProjectionFreshnessGuard>,
    ) -> Self {
        Self {
            delegate,
            tracker: GlobalProjectionTracker::new(projection, freshness_guard),
        }
    }

    fn finish_write(&self) {
        self.tracker.finish_write();
    }
}

impl ModImportResultRepository for ProjectionTrackingModImportResultRepository {
    fn save_new_mod(
        &self,
        logical_mod: &StoredLogicalMod,
        revision: &StoredModRevision,
    ) -> Result<()> {
        let _activity_guard = self.tracker.begin_write()?;
        self.delegate.save_new_mod(logical_mod, revision)?;
        self.finish_write();
        Ok(())
    }

    fn append_revision(&self, revision: &StoredModRevision) -> Result<()> {
        let _activity_guard = self.tracker.begin_write()?;
        self.delegate.append_revision(revision)?;
        self.finish_write();
        Ok(())
    }

    fn upsert_many(&self, upserts: &[ModImportCatalogUpsert]) -> Result<()> {
        if upserts.is_empty() {
            return self.delegate.upsert_many(upserts);
        }
        let _activity_guard = self.tracker.begin_write()?;
        self.delegate.upsert_many(upserts)?;
        self.finish_write();
        Ok(())
    }

    fn upsert_external_import_many(
        &self,
        upserts: &[ModImportExternalCatalogUpsert],
    ) -> Result<()> {
        if upserts.is_empty() {
            return self.delegate.upsert_external_import_many(upserts);
        }
        let _activity_guard = self.tracker.begin_write()?;
        self.delegate.upsert_external_import_many(upserts)?;
        self.finish_write();
        Ok(())
    }

    fn catalog_snapshot(&self) -> Result<ModImportCatalogSnapshot> {
        self.delegate.catalog_snapshot()
    }

    fn get_mod(&self, mod_id: &ModId) -> Result<Option<StoredLogicalMod>> {
        self.delegate.get_mod(mod_id)
    }

    fn list_mods(&self) -> Result<Vec<StoredLogicalMod>> {
        self.delegate.list_mods()
    }

    fn get_revision(&self, revision_id: &ModRevisionId) -> Result<Option<StoredModRevision>> {
        self.delegate.get_revision(revision_id)
    }

    fn list_revisions(&self, mod_id: &ModId) -> Result<Vec<StoredModRevision>> {
        self.delegate.list_revisions(mod_id)
    }

    fn save_analysis(&self, analysis: &StoredModImportAnalysis) -> Result<()> {
        let _activity_guard = self.tracker.begin_write()?;
        self.delegate.save_analysis(analysis)?;
        self.finish_write();
        Ok(())
    }

    fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>> {
        self.delegate.list_analysis()
    }

    fn get_analysis(&self, mod_id: &str) -> Result<Option<StoredModImportAnalysis>> {
        self.delegate.get_analysis(mod_id)
    }
}

pub struct ProjectionTrackingModMetadataRepository {
    delegate: Arc<dyn ModMetadataRepository>,
    tracker: GlobalProjectionTracker,
}

impl ProjectionTrackingModMetadataRepository {
    pub fn new(
        delegate: Arc<dyn ModMetadataRepository>,
        projection: Arc<dyn ModLibraryProjectionRepository>,
        freshness_guard: Arc<ModLibraryProjectionFreshnessGuard>,
    ) -> Self {
        Self {
            delegate,
            tracker: GlobalProjectionTracker::new(projection, freshness_guard),
        }
    }
}

impl ModMetadataRepository for ProjectionTrackingModMetadataRepository {
    fn get(&self, mod_id: &str) -> Result<Option<ModMetadataOverlay>> {
        self.delegate.get(mod_id)
    }

    fn save(&self, overlay: &ModMetadataOverlay) -> Result<()> {
        let _activity_guard = self.tracker.begin_write()?;
        self.delegate.save(overlay)?;
        self.tracker.finish_write();
        Ok(())
    }

    fn delete(&self, mod_id: &str) -> Result<()> {
        let _activity_guard = self.tracker.begin_write()?;
        self.delegate.delete(mod_id)?;
        self.tracker.finish_write();
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<ModMetadataOverlay>> {
        self.delegate.list_all()
    }
}

pub struct ProjectionTrackingCategoryRepository {
    delegate: Arc<dyn CategoryRepository>,
    tracker: GlobalProjectionTracker,
}

impl ProjectionTrackingCategoryRepository {
    pub fn new(
        delegate: Arc<dyn CategoryRepository>,
        projection: Arc<dyn ModLibraryProjectionRepository>,
        freshness_guard: Arc<ModLibraryProjectionFreshnessGuard>,
    ) -> Self {
        Self {
            delegate,
            tracker: GlobalProjectionTracker::new(projection, freshness_guard),
        }
    }
}

impl CategoryRepository for ProjectionTrackingCategoryRepository {
    fn get(&self, category_id: &str) -> Result<Option<Category>> {
        self.delegate.get(category_id)
    }

    fn save(&self, category: &Category) -> Result<()> {
        let _activity_guard = self.tracker.begin_write()?;
        self.delegate.save(category)?;
        self.tracker.finish_write();
        Ok(())
    }

    fn delete(&self, category_id: &str) -> Result<()> {
        let _activity_guard = self.tracker.begin_write()?;
        self.delegate.delete(category_id)?;
        self.tracker.finish_write();
        Ok(())
    }

    fn list_all(&self) -> Result<Vec<Category>> {
        self.delegate.list_all()
    }

    fn count_mods(&self, category_id: &str) -> Result<u32> {
        self.delegate.count_mods(category_id)
    }

    fn get_mod_categories(&self, mod_id: &str) -> Result<Vec<Category>> {
        self.delegate.get_mod_categories(mod_id)
    }

    fn set_mod_categories(&self, mod_id: &str, category_ids: &[String]) -> Result<()> {
        let _activity_guard = self.tracker.begin_write()?;
        self.delegate.set_mod_categories(mod_id, category_ids)?;
        self.tracker.finish_write();
        Ok(())
    }

    fn list_mod_category_pairs(&self) -> Result<Vec<(String, Category)>> {
        self.delegate.list_mod_category_pairs()
    }
}

pub struct ProjectionTrackingInstallManifestRepository {
    delegate: Arc<dyn InstallManifestRepository>,
    projection: Arc<dyn ModLibraryProjectionRepository>,
    freshness_guard: Arc<ModLibraryProjectionFreshnessGuard>,
}

impl ProjectionTrackingInstallManifestRepository {
    pub fn new(
        delegate: Arc<dyn InstallManifestRepository>,
        projection: Arc<dyn ModLibraryProjectionRepository>,
        freshness_guard: Arc<ModLibraryProjectionFreshnessGuard>,
    ) -> Self {
        Self {
            delegate,
            projection,
            freshness_guard,
        }
    }
}

impl InstallManifestRepository for ProjectionTrackingInstallManifestRepository {
    fn load_manifest(&self, profile_id: &ProfileId) -> Result<Option<InstallManifest>> {
        self.delegate.load_manifest(profile_id)
    }

    fn save_manifest(&self, manifest: &InstallManifest) -> Result<()> {
        let activity_guard = self.freshness_guard.begin_authoritative_write();
        if activity_guard.is_err() {
            self.freshness_guard
                .mark_profile_unavailable(&manifest.profile_id);
            self.delegate.save_manifest(manifest)?;
            return Ok(());
        }
        let _activity_guard = activity_guard.expect("checked is_err");
        if self
            .projection
            .mark_profile_dirty(&manifest.profile_id, None)
            .is_err()
        {
            self.freshness_guard
                .mark_profile_unavailable(&manifest.profile_id);
        } else {
            self.freshness_guard.clear_profile(&manifest.profile_id);
        }
        self.delegate.save_manifest(manifest)?;
        if self
            .projection
            .mark_profile_dirty(&manifest.profile_id, None)
            .is_err()
        {
            self.freshness_guard
                .mark_profile_unavailable(&manifest.profile_id);
        } else {
            self.freshness_guard.clear_profile(&manifest.profile_id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{Category, PreviewImageRejectionReason};
    use hmm_ports::{
        ModImportCatalogUpsert, ModLibraryProfileProjection, ModLibraryProfileProjectionState,
        ModLibraryProjectionReadiness, ModLibraryProjectionSnapshot, ModLibraryProjectionState,
        StoredImportPreviewImage, StoredLogicalMod, StoredModOriginProvenance,
        StoredModPackageMetadata, StoredModRevision,
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    struct FakeProjectionRepository {
        fail_global_dirty_calls: HashSet<usize>,
        fail_profile_dirty_calls: HashSet<usize>,
        global_dirty_calls: AtomicUsize,
        profile_dirty_calls: AtomicUsize,
    }

    impl FakeProjectionRepository {
        fn available() -> Self {
            Self {
                fail_global_dirty_calls: HashSet::new(),
                fail_profile_dirty_calls: HashSet::new(),
                global_dirty_calls: AtomicUsize::new(0),
                profile_dirty_calls: AtomicUsize::new(0),
            }
        }

        fn failing_global_dirty() -> Self {
            Self {
                fail_global_dirty_calls: HashSet::from([1, 2, 3]),
                fail_profile_dirty_calls: HashSet::new(),
                global_dirty_calls: AtomicUsize::new(0),
                profile_dirty_calls: AtomicUsize::new(0),
            }
        }

        fn failing_second_global_dirty() -> Self {
            Self {
                fail_global_dirty_calls: HashSet::from([2]),
                fail_profile_dirty_calls: HashSet::new(),
                global_dirty_calls: AtomicUsize::new(0),
                profile_dirty_calls: AtomicUsize::new(0),
            }
        }

        fn failing_second_profile_dirty() -> Self {
            Self {
                fail_global_dirty_calls: HashSet::new(),
                fail_profile_dirty_calls: HashSet::from([2]),
                global_dirty_calls: AtomicUsize::new(0),
                profile_dirty_calls: AtomicUsize::new(0),
            }
        }
    }

    impl ModLibraryProjectionRepository for FakeProjectionRepository {
        fn state(&self) -> Result<ModLibraryProjectionState> {
            Ok(ModLibraryProjectionState {
                schema_version: 1,
                key_version: "test".to_owned(),
                generation: 0,
                source_fingerprint: None,
                readiness: ModLibraryProjectionReadiness::Dirty,
            })
        }

        fn mark_dirty(&self, _observed_source_fingerprint: Option<&str>) -> Result<()> {
            let call = self.global_dirty_calls.fetch_add(1, Ordering::SeqCst) + 1;
            if self.fail_global_dirty_calls.contains(&call) {
                anyhow::bail!("projection dirty marker unavailable");
            }
            Ok(())
        }

        fn rebuild(
            &self,
            _snapshot: &ModLibraryProjectionSnapshot,
        ) -> Result<ModLibraryProjectionState> {
            anyhow::bail!("not used by tracking decorator tests")
        }

        fn profile_state(
            &self,
            _profile_id: &ProfileId,
        ) -> Result<Option<ModLibraryProfileProjectionState>> {
            Ok(None)
        }

        fn mark_profile_dirty(
            &self,
            _profile_id: &ProfileId,
            _observed_source_fingerprint: Option<&str>,
        ) -> Result<()> {
            let call = self.profile_dirty_calls.fetch_add(1, Ordering::SeqCst) + 1;
            if self.fail_profile_dirty_calls.contains(&call) {
                anyhow::bail!("profile projection dirty marker unavailable");
            }
            Ok(())
        }

        fn replace_profile(
            &self,
            _projection: &ModLibraryProfileProjection,
        ) -> Result<ModLibraryProfileProjectionState> {
            anyhow::bail!("not used by tracking decorator tests")
        }
    }

    #[derive(Default)]
    struct RecordingCatalogRepository {
        wrote: AtomicBool,
    }

    impl ModImportResultRepository for RecordingCatalogRepository {
        fn upsert_many(&self, _upserts: &[ModImportCatalogUpsert]) -> Result<()> {
            self.wrote.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> Result<()> {
            self.wrote.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>> {
            Ok(Vec::new())
        }

        fn get_analysis(&self, _mod_id: &str) -> Result<Option<StoredModImportAnalysis>> {
            Ok(None)
        }
    }

    #[derive(Default)]
    struct PartiallyFailingCatalogRepository {
        committed_first_chunk: AtomicBool,
    }

    impl ModImportResultRepository for PartiallyFailingCatalogRepository {
        fn upsert_many(&self, _upserts: &[ModImportCatalogUpsert]) -> Result<()> {
            // Models a JSON repository that durably committed an earlier chunk before a later one failed.
            self.committed_first_chunk.store(true, Ordering::SeqCst);
            anyhow::bail!("simulated later catalog chunk failure")
        }

        fn save_analysis(&self, _analysis: &StoredModImportAnalysis) -> Result<()> {
            anyhow::bail!("not used by this test repository")
        }

        fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>> {
            Ok(Vec::new())
        }

        fn get_analysis(&self, _mod_id: &str) -> Result<Option<StoredModImportAnalysis>> {
            Ok(None)
        }
    }

    #[derive(Default)]
    struct RecordingMetadataRepository {
        wrote: AtomicBool,
    }

    impl ModMetadataRepository for RecordingMetadataRepository {
        fn get(&self, _mod_id: &str) -> Result<Option<ModMetadataOverlay>> {
            Ok(None)
        }

        fn save(&self, _overlay: &ModMetadataOverlay) -> Result<()> {
            self.wrote.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn delete(&self, _mod_id: &str) -> Result<()> {
            Ok(())
        }

        fn list_all(&self) -> Result<Vec<ModMetadataOverlay>> {
            Ok(Vec::new())
        }
    }

    #[derive(Default)]
    struct RecordingCategoryRepository {
        wrote: AtomicBool,
    }

    impl CategoryRepository for RecordingCategoryRepository {
        fn get(&self, _category_id: &str) -> Result<Option<Category>> {
            Ok(None)
        }

        fn save(&self, _category: &Category) -> Result<()> {
            self.wrote.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn delete(&self, _category_id: &str) -> Result<()> {
            Ok(())
        }

        fn list_all(&self) -> Result<Vec<Category>> {
            Ok(Vec::new())
        }

        fn count_mods(&self, _category_id: &str) -> Result<u32> {
            Ok(0)
        }

        fn get_mod_categories(&self, _mod_id: &str) -> Result<Vec<Category>> {
            Ok(Vec::new())
        }

        fn set_mod_categories(&self, _mod_id: &str, _category_ids: &[String]) -> Result<()> {
            Ok(())
        }

        fn list_mod_category_pairs(&self) -> Result<Vec<(String, Category)>> {
            Ok(Vec::new())
        }
    }

    #[derive(Default)]
    struct RecordingManifestRepository {
        manifests: Mutex<Vec<InstallManifest>>,
    }

    impl InstallManifestRepository for RecordingManifestRepository {
        fn load_manifest(&self, _profile_id: &ProfileId) -> Result<Option<InstallManifest>> {
            Ok(None)
        }

        fn save_manifest(&self, manifest: &InstallManifest) -> Result<()> {
            self.manifests
                .lock()
                .expect("manifest lock")
                .push(manifest.clone());
            Ok(())
        }
    }

    fn analysis() -> StoredModImportAnalysis {
        StoredModImportAnalysis {
            mod_id: "mod-a".to_owned(),
            task_id: "task-a".to_owned(),
            package_id: "package-a".to_owned(),
            display_name: "Alpha".to_owned(),
            metadata: StoredModPackageMetadata::default(),
            preview_image: StoredImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::Missing,
            },
        }
    }

    fn overlay() -> ModMetadataOverlay {
        ModMetadataOverlay {
            mod_id: ModId::new("mod-a"),
            display_name: Some("Alpha".to_owned()),
            author: None,
            version: None,
            description: None,
            nexus_mod_id: None,
            updated_at: 1,
        }
    }

    fn category() -> Category {
        Category {
            id: "category-a".to_owned(),
            name: "Armor".to_owned(),
            color: None,
            sort_order: 0,
            created_at: 1,
        }
    }

    #[test]
    fn catalog_metadata_and_category_writes_stop_before_authoritative_write_when_dirty_marking_fails(
    ) {
        let projection = Arc::new(FakeProjectionRepository::failing_global_dirty());
        let catalog = Arc::new(RecordingCatalogRepository::default());
        let metadata = Arc::new(RecordingMetadataRepository::default());
        let categories = Arc::new(RecordingCategoryRepository::default());
        let freshness_guard = Arc::new(ModLibraryProjectionFreshnessGuard::default());

        let catalog_tracking = ProjectionTrackingModImportResultRepository::new(
            catalog.clone(),
            projection.clone(),
            freshness_guard.clone(),
        );
        let metadata_tracking = ProjectionTrackingModMetadataRepository::new(
            metadata.clone(),
            projection.clone(),
            freshness_guard.clone(),
        );
        let category_tracking = ProjectionTrackingCategoryRepository::new(
            categories.clone(),
            projection.clone(),
            freshness_guard,
        );

        assert!(catalog_tracking.save_analysis(&analysis()).is_err());
        assert!(metadata_tracking.save(&overlay()).is_err());
        assert!(category_tracking.save(&category()).is_err());
        assert!(!catalog.wrote.load(Ordering::SeqCst));
        assert!(!metadata.wrote.load(Ordering::SeqCst));
        assert!(!categories.wrote.load(Ordering::SeqCst));
        assert_eq!(projection.global_dirty_calls.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn global_write_remains_successful_and_fails_closed_when_post_commit_dirty_marking_fails() {
        let projection = Arc::new(FakeProjectionRepository::failing_second_global_dirty());
        let delegate = Arc::new(RecordingCatalogRepository::default());
        let freshness_guard = Arc::new(ModLibraryProjectionFreshnessGuard::default());
        let repository = ProjectionTrackingModImportResultRepository::new(
            delegate.clone(),
            projection.clone(),
            freshness_guard.clone(),
        );

        repository
            .save_analysis(&analysis())
            .expect("catalog fact stays durable when projection freshness update fails");

        assert!(delegate.wrote.load(Ordering::SeqCst));
        assert_eq!(projection.global_dirty_calls.load(Ordering::SeqCst), 2);
        assert!(freshness_guard.global_is_unavailable());
    }

    #[test]
    fn catalog_upsert_many_remains_durable_and_fails_closed_when_post_commit_dirty_marking_fails() {
        let projection = Arc::new(FakeProjectionRepository::failing_second_global_dirty());
        let delegate = Arc::new(RecordingCatalogRepository::default());
        let freshness_guard = Arc::new(ModLibraryProjectionFreshnessGuard::default());
        let repository = ProjectionTrackingModImportResultRepository::new(
            delegate.clone(),
            projection.clone(),
            freshness_guard.clone(),
        );
        let revision_id = ModRevisionId::new("revision-a");
        let upsert = ModImportCatalogUpsert {
            logical_mod: StoredLogicalMod {
                mod_id: ModId::new("mod-a"),
                origin_revision_id: revision_id.clone(),
                display_revision_id: revision_id.clone(),
                origin_provenance: StoredModOriginProvenance::Imported,
            },
            revision: StoredModRevision {
                revision_id,
                mod_id: ModId::new("mod-a"),
                import_task_id: "task-a".to_owned(),
                package_id: "package-a".to_owned(),
                display_name: "Alpha".to_owned(),
                metadata: StoredModPackageMetadata::default(),
                preview_image: StoredImportPreviewImage::Fallback {
                    reason: PreviewImageRejectionReason::Missing,
                },
            },
        };

        repository
            .upsert_many(&[upsert])
            .expect("catalog fact remains durable when freshness update fails");

        assert!(delegate.wrote.load(Ordering::SeqCst));
        assert_eq!(projection.global_dirty_calls.load(Ordering::SeqCst), 2);
        assert!(freshness_guard.global_is_unavailable());
    }

    #[test]
    fn catalog_upsert_many_keeps_projection_dirty_when_a_later_chunk_fails() {
        let projection = Arc::new(FakeProjectionRepository::available());
        let delegate = Arc::new(PartiallyFailingCatalogRepository::default());
        let freshness_guard = Arc::new(ModLibraryProjectionFreshnessGuard::default());
        let repository = ProjectionTrackingModImportResultRepository::new(
            delegate.clone(),
            projection.clone(),
            freshness_guard.clone(),
        );

        let error = repository
            .upsert_many(&[ModImportCatalogUpsert {
                logical_mod: StoredLogicalMod {
                    mod_id: ModId::new("mod-a"),
                    origin_revision_id: ModRevisionId::new("revision-a"),
                    display_revision_id: ModRevisionId::new("revision-a"),
                    origin_provenance: StoredModOriginProvenance::Imported,
                },
                revision: StoredModRevision {
                    revision_id: ModRevisionId::new("revision-a"),
                    mod_id: ModId::new("mod-a"),
                    import_task_id: "task-a".to_owned(),
                    package_id: "package-a".to_owned(),
                    display_name: "Alpha".to_owned(),
                    metadata: StoredModPackageMetadata::default(),
                    preview_image: StoredImportPreviewImage::Fallback {
                        reason: PreviewImageRejectionReason::Missing,
                    },
                },
            }])
            .expect_err("a later catalog chunk failure remains visible to the caller");

        assert!(error
            .to_string()
            .contains("simulated later catalog chunk failure"));
        assert!(delegate.committed_first_chunk.load(Ordering::SeqCst));
        assert_eq!(projection.global_dirty_calls.load(Ordering::SeqCst), 1);
        assert!(!freshness_guard.global_is_unavailable());
    }

    #[test]
    fn manifest_save_remains_successful_when_post_commit_dirty_marking_fails() {
        let projection = Arc::new(FakeProjectionRepository::failing_second_profile_dirty());
        let delegate = Arc::new(RecordingManifestRepository::default());
        let freshness_guard = Arc::new(ModLibraryProjectionFreshnessGuard::default());
        let repository = ProjectionTrackingInstallManifestRepository::new(
            delegate.clone(),
            projection.clone(),
            freshness_guard.clone(),
        );
        let manifest = InstallManifest::completed(ProfileId::new("profile-a"), Vec::new());

        repository
            .save_manifest(&manifest)
            .expect("manifest fact stays durable when projection freshness update fails");

        assert_eq!(
            delegate.manifests.lock().expect("manifest lock").as_slice(),
            &[manifest]
        );
        assert_eq!(projection.profile_dirty_calls.load(Ordering::SeqCst), 2);
        assert!(freshness_guard.profile_is_unavailable(&ProfileId::new("profile-a")));
    }

    #[test]
    fn refresh_waits_for_the_entire_authoritative_write_window() {
        let freshness_guard = Arc::new(ModLibraryProjectionFreshnessGuard::default());
        let write_guard = freshness_guard
            .begin_authoritative_write()
            .expect("authoritative write guard");
        let (started, received) = mpsc::channel();
        let refresh_guard = Arc::clone(&freshness_guard);
        let refresh = thread::spawn(move || {
            let _refresh_guard = refresh_guard.begin_refresh().expect("refresh guard");
            started.send(()).expect("refresh start signal");
        });

        assert!(received.recv_timeout(Duration::from_millis(50)).is_err());
        drop(write_guard);
        received
            .recv_timeout(Duration::from_secs(1))
            .expect("refresh proceeds after authoritative write");
        refresh.join().expect("refresh thread");
    }
}
