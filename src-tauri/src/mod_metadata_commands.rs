use crate::dto::CommandErrorDto;
use crate::state::AppState;
use hmm_app::UpdateModMetadataRequest;
use tauri::State;

#[tauri::command]
pub fn update_mod_metadata(
    mod_id: String,
    display_name: Option<String>,
    author: Option<String>,
    version: Option<String>,
    description: Option<String>,
    nexus_mod_id: Option<u64>,
    state: State<'_, AppState>,
) -> Result<(), CommandErrorDto> {
    let mod_id = parse_mod_id(mod_id)?;

    state
        .mod_metadata
        .update_metadata(UpdateModMetadataRequest {
            mod_id,
            display_name,
            author,
            version,
            description,
            nexus_mod_id,
        })
        .map_err(|_| metadata_unavailable_error())
}

#[tauri::command]
pub fn delete_mod_metadata(
    mod_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandErrorDto> {
    let mod_id = parse_mod_id(mod_id)?;

    state
        .mod_metadata
        .delete_metadata(&mod_id)
        .map_err(|_| metadata_unavailable_error())
}

fn parse_mod_id(value: String) -> Result<String, CommandErrorDto> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(CommandErrorDto {
            code: "mod_id_empty".to_owned(),
            message: "mod id cannot be empty".to_owned(),
        });
    }

    Ok(trimmed.to_owned())
}

fn metadata_unavailable_error() -> CommandErrorDto {
    CommandErrorDto {
        code: "mod_metadata_unavailable".to_owned(),
        message: "mod metadata storage is unavailable".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mod_id_rejects_empty() {
        let result = parse_mod_id("".to_owned());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, "mod_id_empty");
    }

    #[test]
    fn parse_mod_id_trims_whitespace() {
        assert_eq!(parse_mod_id("  mod-1  ".to_owned()).unwrap(), "mod-1");
    }

    /// End-to-end: real SQLite → SqliteModMetadataRepository →
    /// ModMetadataService.update → ModLibraryService.get_mod_library
    /// confirms overlay merges into library items.
    #[test]
    fn overlay_merges_with_real_sqlite() {
        use hmm_app::{ModLibraryService, ModMetadataService};
        use hmm_core::PreviewImageRejectionReason;
        use hmm_infra::{SqliteCategoryRepository, SqliteModMetadataRepository};
        use hmm_ports::{
            ModImportResultRepository, StoredImportPreviewImage, StoredModImportAnalysis,
            StoredModPackageMetadata,
        };
        use std::sync::{Arc, Mutex};

        // Set up real SQLite database
        let temp = tempfile::tempdir().expect("temp dir");
        let db_path = temp.path().join("test.db");
        let conn = hmm_infra::open_database(&db_path).expect("open db");
        let db = Arc::new(Mutex::new(conn));
        let metadata_repo = Arc::new(SqliteModMetadataRepository::new(Arc::clone(&db)));
        let category_repo = Arc::new(SqliteCategoryRepository::new(Arc::clone(&db)));

        // Set up a fake import result repository with one mod
        struct InMemoryResultRepo(Mutex<Vec<StoredModImportAnalysis>>);
        impl ModImportResultRepository for InMemoryResultRepo {
            fn save_analysis(&self, a: &StoredModImportAnalysis) -> anyhow::Result<()> {
                let mut v = self.0.lock().unwrap();
                v.retain(|r| r.mod_id != a.mod_id);
                v.push(a.clone());
                Ok(())
            }
            fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
                Ok(self.0.lock().unwrap().clone())
            }
            fn get_analysis(&self, id: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
                Ok(self
                    .0
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|r| r.mod_id == id)
                    .cloned())
            }
        }

        let result_repo = Arc::new(InMemoryResultRepo(Mutex::new(vec![])));
        result_repo
            .save_analysis(&StoredModImportAnalysis {
                mod_id: "mod-abc".to_owned(),
                task_id: "task-1".to_owned(),
                package_id: "mod-abc".to_owned(),
                display_name: "Original Name".to_owned(),
                metadata: StoredModPackageMetadata {
                    version: Some("1.0.0".to_owned()),
                    author: Some("Original Author".to_owned()),
                    category: None,
                    tags: vec![],
                    dependencies: vec![],
                },
                preview_image: StoredImportPreviewImage::Fallback {
                    reason: PreviewImageRejectionReason::Missing,
                },
            })
            .unwrap();

        // Before overlay: library shows original values
        let library_service = ModLibraryService::new(
            Arc::clone(&result_repo) as _,
            Arc::clone(&metadata_repo) as _,
            Arc::clone(&category_repo) as _,
        );
        let before = library_service.get_mod_library().unwrap();
        assert_eq!(before[0].name, "Original Name");
        assert_eq!(before[0].author.as_deref(), Some("Original Author"));

        // Write overlay via ModMetadataService (same path as update_mod_metadata command)
        struct FixedClock;
        impl hmm_ports::AppClock for FixedClock {
            fn now_unix_millis(&self) -> anyhow::Result<u128> {
                Ok(9999)
            }
        }
        let metadata_service =
            ModMetadataService::new(Arc::clone(&metadata_repo) as _, Arc::new(FixedClock));
        metadata_service
            .update_metadata(UpdateModMetadataRequest {
                mod_id: "mod-abc".to_owned(),
                display_name: Some("User Edited Name".to_owned()),
                author: Some("New Author".to_owned()),
                version: Some("2.5.0".to_owned()),
                description: Some("My personal notes".to_owned()),
                nexus_mod_id: Some(42),
            })
            .unwrap();

        // After overlay: library shows merged values
        let after = library_service.get_mod_library().unwrap();
        assert_eq!(after[0].name, "User Edited Name");
        assert_eq!(after[0].author.as_deref(), Some("New Author"));
        assert_eq!(after[0].version_label.as_deref(), Some("v2.5.0"));

        // Detail also shows overlay fields
        let detail = library_service.get_mod_detail("mod-abc").unwrap().unwrap();
        assert_eq!(detail.name, "User Edited Name");
        assert_eq!(detail.metadata.version.as_deref(), Some("2.5.0"));
        assert_eq!(detail.description.as_deref(), Some("My personal notes"));
        assert_eq!(detail.nexus_mod_id, Some(42));

        // Delete overlay → reverts to original
        metadata_service.delete_metadata("mod-abc").unwrap();
        let reverted = library_service.get_mod_library().unwrap();
        assert_eq!(reverted[0].name, "Original Name");
        assert_eq!(reverted[0].author.as_deref(), Some("Original Author"));
        assert_eq!(reverted[0].version_label.as_deref(), Some("v1.0.0"));
    }
}
