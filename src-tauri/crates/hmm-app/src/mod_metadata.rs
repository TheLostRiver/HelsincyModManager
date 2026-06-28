use anyhow::Result;
use hmm_core::{ModId, ModMetadataOverlay};
use hmm_ports::{AppClock, ModMetadataRepository};
use std::sync::Arc;

pub struct UpdateModMetadataRequest {
    pub mod_id: String,
    pub display_name: Option<String>,
    pub author: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub nexus_mod_id: Option<u64>,
}

pub struct ModMetadataService {
    metadata_repository: Arc<dyn ModMetadataRepository>,
    clock: Arc<dyn AppClock>,
}

impl ModMetadataService {
    pub fn new(
        metadata_repository: Arc<dyn ModMetadataRepository>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            metadata_repository,
            clock,
        }
    }

    pub fn update_metadata(&self, request: UpdateModMetadataRequest) -> Result<()> {
        let now = self.clock.now_unix_millis()?;
        let overlay = ModMetadataOverlay {
            mod_id: ModId::new(&request.mod_id),
            display_name: normalize_optional_string(request.display_name),
            author: normalize_optional_string(request.author),
            version: normalize_optional_string(request.version),
            description: normalize_optional_string(request.description),
            nexus_mod_id: request.nexus_mod_id,
            updated_at: now,
        };
        self.metadata_repository.save(&overlay)
    }

    pub fn delete_metadata(&self, mod_id: &str) -> Result<()> {
        self.metadata_repository.delete(mod_id)
    }

    pub fn get_metadata(&self, mod_id: &str) -> Result<Option<ModMetadataOverlay>> {
        self.metadata_repository.get(mod_id)
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeMetadataRepository {
        overlays: Mutex<Vec<ModMetadataOverlay>>,
    }

    impl FakeMetadataRepository {
        fn new() -> Self {
            Self {
                overlays: Mutex::new(Vec::new()),
            }
        }
    }

    impl ModMetadataRepository for FakeMetadataRepository {
        fn get(&self, mod_id: &str) -> Result<Option<ModMetadataOverlay>> {
            let overlays = self.overlays.lock().unwrap();
            Ok(overlays
                .iter()
                .find(|o| o.mod_id.as_str() == mod_id)
                .cloned())
        }

        fn save(&self, overlay: &ModMetadataOverlay) -> Result<()> {
            let mut overlays = self.overlays.lock().unwrap();
            overlays.retain(|o| o.mod_id.as_str() != overlay.mod_id.as_str());
            overlays.push(overlay.clone());
            Ok(())
        }

        fn delete(&self, mod_id: &str) -> Result<()> {
            let mut overlays = self.overlays.lock().unwrap();
            overlays.retain(|o| o.mod_id.as_str() != mod_id);
            Ok(())
        }

        fn list_all(&self) -> Result<Vec<ModMetadataOverlay>> {
            Ok(self.overlays.lock().unwrap().clone())
        }
    }

    struct FixedClock(u128);

    impl AppClock for FixedClock {
        fn now_unix_millis(&self) -> Result<u128> {
            Ok(self.0)
        }
    }

    #[test]
    fn update_metadata_saves_overlay_with_clock_timestamp() {
        let repo = Arc::new(FakeMetadataRepository::new());
        let clock = Arc::new(FixedClock(5000));
        let service = ModMetadataService::new(Arc::clone(&repo) as _, clock);

        service
            .update_metadata(UpdateModMetadataRequest {
                mod_id: "mod-1".to_owned(),
                display_name: Some("Custom".to_owned()),
                author: None,
                version: None,
                description: None,
                nexus_mod_id: None,
            })
            .unwrap();

        let overlay = repo.get("mod-1").unwrap().expect("should exist");
        assert_eq!(overlay.display_name, Some("Custom".to_owned()));
        assert_eq!(overlay.updated_at, 5000);
    }

    #[test]
    fn delete_metadata_removes_overlay() {
        let repo = Arc::new(FakeMetadataRepository::new());
        let clock = Arc::new(FixedClock(1000));
        let service = ModMetadataService::new(Arc::clone(&repo) as _, clock);

        service
            .update_metadata(UpdateModMetadataRequest {
                mod_id: "mod-1".to_owned(),
                display_name: Some("Name".to_owned()),
                author: None,
                version: None,
                description: None,
                nexus_mod_id: None,
            })
            .unwrap();

        service.delete_metadata("mod-1").unwrap();
        assert!(repo.get("mod-1").unwrap().is_none());
    }

    #[test]
    fn get_metadata_returns_none_when_not_found() {
        let repo = Arc::new(FakeMetadataRepository::new());
        let clock = Arc::new(FixedClock(1000));
        let service = ModMetadataService::new(repo, clock);

        assert!(service.get_metadata("nonexistent").unwrap().is_none());
    }

    #[test]
    fn update_metadata_normalizes_empty_strings_to_none() {
        let repo = Arc::new(FakeMetadataRepository::new());
        let clock = Arc::new(FixedClock(1000));
        let service = ModMetadataService::new(Arc::clone(&repo) as _, clock);

        service
            .update_metadata(UpdateModMetadataRequest {
                mod_id: "mod-1".to_owned(),
                display_name: Some("".to_owned()),
                author: Some("   ".to_owned()),
                version: Some("  ".to_owned()),
                description: Some("".to_owned()),
                nexus_mod_id: Some(42),
            })
            .unwrap();

        let overlay = repo.get("mod-1").unwrap().expect("should exist");
        assert!(overlay.display_name.is_none());
        assert!(overlay.author.is_none());
        assert!(overlay.version.is_none());
        assert!(overlay.description.is_none());
        assert_eq!(overlay.nexus_mod_id, Some(42));
    }

    #[test]
    fn update_metadata_trims_whitespace_from_strings() {
        let repo = Arc::new(FakeMetadataRepository::new());
        let clock = Arc::new(FixedClock(1000));
        let service = ModMetadataService::new(Arc::clone(&repo) as _, clock);

        service
            .update_metadata(UpdateModMetadataRequest {
                mod_id: "mod-1".to_owned(),
                display_name: Some("  Trimmed Name  ".to_owned()),
                author: Some("  Author  ".to_owned()),
                version: None,
                description: None,
                nexus_mod_id: None,
            })
            .unwrap();

        let overlay = repo.get("mod-1").unwrap().expect("should exist");
        assert_eq!(overlay.display_name.as_deref(), Some("Trimmed Name"));
        assert_eq!(overlay.author.as_deref(), Some("Author"));
    }

    // — overlay merge integration tests —

    use crate::ModLibraryService;
    use hmm_core::{Category, PreviewImageRejectionReason};
    use hmm_ports::{
        CategoryRepository, ModImportResultRepository, StoredImportPreviewImage,
        StoredModImportAnalysis, StoredModPackageMetadata,
    };

    struct EmptyCategoryRepository;

    impl CategoryRepository for EmptyCategoryRepository {
        fn get(&self, _: &str) -> Result<Option<Category>> { Ok(None) }
        fn save(&self, _: &Category) -> Result<()> { Ok(()) }
        fn delete(&self, _: &str) -> Result<()> { Ok(()) }
        fn list_all(&self) -> Result<Vec<Category>> { Ok(Vec::new()) }
        fn count_mods(&self, _: &str) -> Result<u32> { Ok(0) }
        fn get_mod_categories(&self, _: &str) -> Result<Vec<Category>> { Ok(Vec::new()) }
        fn set_mod_categories(&self, _: &str, _: &[String]) -> Result<()> { Ok(()) }
        fn list_mod_category_pairs(&self) -> Result<Vec<(String, Category)>> { Ok(Vec::new()) }
    }

    #[derive(Default)]
    struct FakeResultRepository {
        records: Mutex<Vec<StoredModImportAnalysis>>,
    }

    impl ModImportResultRepository for FakeResultRepository {
        fn save_analysis(&self, analysis: &StoredModImportAnalysis) -> Result<()> {
            let mut records = self.records.lock().unwrap();
            records.retain(|r| r.mod_id != analysis.mod_id);
            records.push(analysis.clone());
            Ok(())
        }
        fn list_analysis(&self) -> Result<Vec<StoredModImportAnalysis>> {
            Ok(self.records.lock().unwrap().clone())
        }
        fn get_analysis(&self, mod_id: &str) -> Result<Option<StoredModImportAnalysis>> {
            Ok(self.records.lock().unwrap().iter().find(|r| r.mod_id == mod_id).cloned())
        }
    }

    fn sample_analysis(mod_id: &str) -> StoredModImportAnalysis {
        StoredModImportAnalysis {
            mod_id: mod_id.to_owned(),
            task_id: "task-1".to_owned(),
            package_id: mod_id.to_owned(),
            display_name: mod_id.to_owned(),
            metadata: StoredModPackageMetadata {
                version: Some("1.0.0".to_owned()),
                author: Some("Author".to_owned()),
                category: None,
                tags: vec![],
                dependencies: vec![],
            },
            preview_image: StoredImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::Missing,
            },
        }
    }

    #[test]
    fn overlay_merges_into_library_item() {
        let result_repo = Arc::new(FakeResultRepository::default());
        result_repo.save_analysis(&sample_analysis("pkg-1")).unwrap();

        let metadata_repo = Arc::new(FakeMetadataRepository::new());
        metadata_repo
            .save(&ModMetadataOverlay {
                mod_id: ModId::new("pkg-1"),
                display_name: Some("Custom Name".to_owned()),
                author: Some("Custom Author".to_owned()),
                version: Some("2.0.0".to_owned()),
                description: None,
                nexus_mod_id: None,
                updated_at: 1000,
            })
            .unwrap();

        let service = ModLibraryService::new(result_repo, metadata_repo, Arc::new(EmptyCategoryRepository));
        let library = service.get_mod_library().unwrap();

        assert_eq!(library[0].name, "Custom Name");
        assert_eq!(library[0].author.as_deref(), Some("Custom Author"));
        assert_eq!(library[0].version_label.as_deref(), Some("v2.0.0"));
    }

    #[test]
    fn overlay_merges_into_detail() {
        let result_repo = Arc::new(FakeResultRepository::default());
        result_repo.save_analysis(&sample_analysis("pkg-1")).unwrap();

        let metadata_repo = Arc::new(FakeMetadataRepository::new());
        metadata_repo
            .save(&ModMetadataOverlay {
                mod_id: ModId::new("pkg-1"),
                display_name: Some("Edited".to_owned()),
                author: None,
                version: Some("3.0".to_owned()),
                description: Some("User notes".to_owned()),
                nexus_mod_id: Some(99999),
                updated_at: 2000,
            })
            .unwrap();

        let service = ModLibraryService::new(result_repo, metadata_repo, Arc::new(EmptyCategoryRepository));
        let detail = service.get_mod_detail("pkg-1").unwrap().unwrap();

        assert_eq!(detail.name, "Edited");
        assert_eq!(detail.metadata.author.as_deref(), Some("Author"));
        assert_eq!(detail.metadata.version.as_deref(), Some("3.0"));
        assert_eq!(detail.description.as_deref(), Some("User notes"));
        assert_eq!(detail.nexus_mod_id, Some(99999));
    }

    #[test]
    fn no_overlay_preserves_original_values() {
        let result_repo = Arc::new(FakeResultRepository::default());
        result_repo.save_analysis(&sample_analysis("pkg-1")).unwrap();

        let metadata_repo = Arc::new(FakeMetadataRepository::new());
        let service = ModLibraryService::new(result_repo, metadata_repo, Arc::new(EmptyCategoryRepository));
        let library = service.get_mod_library().unwrap();
        let detail = service.get_mod_detail("pkg-1").unwrap().unwrap();

        assert_eq!(library[0].name, "pkg-1");
        assert_eq!(library[0].author.as_deref(), Some("Author"));
        assert_eq!(detail.name, "pkg-1");
        assert!(detail.description.is_none());
        assert!(detail.nexus_mod_id.is_none());
    }
}
