use hmm_ports::{ModImportResultRepository, ModPackageSizeReader, ModRevisionSizeUpdate};
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Runs once at desktop startup, outside the library query and game-write paths.
pub struct ModLibraryStatisticsService {
    repository: Arc<dyn ModImportResultRepository>,
    size_reader: Arc<dyn ModPackageSizeReader>,
    started: AtomicBool,
}

impl ModLibraryStatisticsService {
    pub fn new(
        repository: Arc<dyn ModImportResultRepository>,
        size_reader: Arc<dyn ModPackageSizeReader>,
    ) -> Self {
        Self {
            repository,
            size_reader,
            started: AtomicBool::new(false),
        }
    }

    pub fn refresh_missing_sizes(&self) -> anyhow::Result<usize> {
        if self.started.swap(true, Ordering::AcqRel) {
            return Ok(0);
        }
        let snapshot = self.repository.catalog_snapshot()?;
        let revisions = snapshot
            .revisions
            .iter()
            .map(|revision| (revision.revision_id.as_str(), revision))
            .collect::<HashMap<_, _>>();
        let mut updates = Vec::new();
        let mut changed = 0;
        for logical_mod in snapshot.logical_mods {
            let Some(revision) = revisions.get(logical_mod.display_revision_id.as_str()) else {
                continue;
            };
            if revision.mod_id != logical_mod.mod_id
                || revision.statistics.content_size_bytes.is_some()
            {
                continue;
            }
            // A missing or unsafe package remains unknown; do not fabricate a zero or block the library.
            if let Ok(size) = self.size_reader.read_content_size(&revision.package_id) {
                updates.push(ModRevisionSizeUpdate {
                    mod_id: revision.mod_id.clone(),
                    revision_id: revision.revision_id.clone(),
                    package_id: revision.package_id.clone(),
                    content_size_bytes: size,
                });
            }
            if updates.len() == hmm_ports::MOD_IMPORT_UPSERT_MAX_ENTRIES {
                changed += self.repository.fill_missing_content_sizes(&updates)?;
                updates.clear();
            }
        }
        changed += self.repository.fill_missing_content_sizes(&updates)?;
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{ModId, ModRevisionId, PreviewImageRejectionReason};
    use hmm_ports::{
        ModImportCatalogSnapshot, StoredImportPreviewImage, StoredLogicalMod,
        StoredModImportAnalysis, StoredModOriginProvenance, StoredModRevision,
    };
    use std::sync::Mutex;

    struct Repository {
        snapshot: ModImportCatalogSnapshot,
        updates: Mutex<Vec<ModRevisionSizeUpdate>>,
    }
    impl ModImportResultRepository for Repository {
        fn catalog_snapshot(&self) -> anyhow::Result<ModImportCatalogSnapshot> {
            Ok(self.snapshot.clone())
        }
        fn fill_missing_content_sizes(
            &self,
            updates: &[ModRevisionSizeUpdate],
        ) -> anyhow::Result<usize> {
            self.updates.lock().unwrap().extend_from_slice(updates);
            Ok(updates.len())
        }
        fn save_analysis(&self, _: &StoredModImportAnalysis) -> anyhow::Result<()> {
            unreachable!()
        }
        fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
            unreachable!()
        }
        fn get_analysis(&self, _: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
            unreachable!()
        }
    }
    struct Reader(Mutex<Vec<String>>);
    impl ModPackageSizeReader for Reader {
        fn read_content_size(&self, id: &str) -> anyhow::Result<u64> {
            self.0.lock().unwrap().push(id.into());
            anyhow::ensure!(id != "missing", "package unavailable");
            Ok(42)
        }
    }
    #[test]
    fn only_missing_display_sizes_are_read_once_and_unavailable_packages_stay_unknown() {
        let mut snapshot = ModImportCatalogSnapshot::default();
        for (id, known) in [
            ("old", Some(1)),
            ("display", None),
            ("known", Some(9)),
            ("missing", None),
        ] {
            snapshot.logical_mods.push(StoredLogicalMod {
                mod_id: ModId::new(id),
                origin_revision_id: ModRevisionId::new(id),
                display_revision_id: ModRevisionId::new(id),
                origin_provenance: StoredModOriginProvenance::Imported,
            });
            snapshot.revisions.push(StoredModRevision {
                revision_id: ModRevisionId::new(id),
                mod_id: ModId::new(id),
                import_task_id: id.into(),
                package_id: id.into(),
                display_name: id.into(),
                statistics: hmm_ports::ModRevisionStatistics {
                    content_size_bytes: known,
                    imported_at_unix_millis: None,
                },
                metadata: Default::default(),
                preview_image: StoredImportPreviewImage::Fallback {
                    reason: PreviewImageRejectionReason::Missing,
                },
            });
        }
        snapshot.logical_mods.remove(0);
        let repository = Arc::new(Repository {
            snapshot,
            updates: Mutex::new(vec![]),
        });
        let reader = Arc::new(Reader(Mutex::new(vec![])));
        let service = ModLibraryStatisticsService::new(repository.clone(), reader.clone());
        assert_eq!(service.refresh_missing_sizes().unwrap(), 1);
        assert_eq!(service.refresh_missing_sizes().unwrap(), 0);
        assert_eq!(*reader.0.lock().unwrap(), vec!["display", "missing"]);
        let updates = repository.updates.lock().unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].package_id, "display");
        assert_eq!(updates[0].content_size_bytes, 42);
    }
}
