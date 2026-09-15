use super::*;
use hmm_core::{ModMetadataOverlay, ModRevisionId};
use hmm_ports::{
    StoredLogicalMod, StoredModImportAnalysis, StoredModOriginProvenance, StoredModRevision,
};
use std::sync::Mutex;

struct Imports;
impl ModImportResultRepository for Imports {
    fn save_analysis(&self, _: &StoredModImportAnalysis) -> anyhow::Result<()> {
        unreachable!()
    }
    fn list_analysis(&self) -> anyhow::Result<Vec<StoredModImportAnalysis>> {
        unreachable!()
    }
    fn get_analysis(&self, _: &str) -> anyhow::Result<Option<StoredModImportAnalysis>> {
        unreachable!()
    }
    fn get_mod(&self, id: &ModId) -> anyhow::Result<Option<StoredLogicalMod>> {
        if id.as_str() == "missing" {
            return Ok(None);
        }
        Ok(Some(StoredLogicalMod {
            mod_id: id.clone(),
            origin_revision_id: ModRevisionId::new("original"),
            display_revision_id: ModRevisionId::new(if id.as_str() == "wrong-owner" {
                "wrong"
            } else {
                "display"
            }),
            origin_provenance: StoredModOriginProvenance::Imported,
        }))
    }
    fn get_revision(&self, id: &ModRevisionId) -> anyhow::Result<Option<StoredModRevision>> {
        Ok(Some(StoredModRevision {
            revision_id: id.clone(),
            mod_id: ModId::new("mod-a"),
            import_task_id: "task".into(),
            package_id: "display-package".into(),
            display_name: "Fixture".into(),
            metadata: Default::default(),
            preview_image: hmm_ports::StoredImportPreviewImage::Fallback {
                reason: hmm_core::PreviewImageRejectionReason::Missing,
            },
        }))
    }
}

struct Metadata(Option<u64>);
impl ModMetadataRepository for Metadata {
    fn get(&self, id: &str) -> anyhow::Result<Option<ModMetadataOverlay>> {
        Ok(Some(ModMetadataOverlay {
            mod_id: ModId::new(id),
            display_name: None,
            author: None,
            version: None,
            description: None,
            nexus_mod_id: self.0,
            updated_at: 0,
        }))
    }
    fn save(&self, _: &ModMetadataOverlay) -> anyhow::Result<()> {
        unreachable!()
    }
    fn delete(&self, _: &str) -> anyhow::Result<()> {
        unreachable!()
    }
    fn list_all(&self) -> anyhow::Result<Vec<ModMetadataOverlay>> {
        unreachable!()
    }
}

#[derive(Default)]
struct Directories(Mutex<Vec<String>>);
impl ModPackageDirectoryOpener for Directories {
    fn open_package_directory(&self, id: &str) -> anyhow::Result<()> {
        self.0.lock().unwrap().push(id.into());
        Ok(())
    }
}

fn service(id: Option<u64>) -> (ModShortcutService, Arc<Directories>) {
    let opener = Arc::new(Directories::default());
    (
        ModShortcutService::new(
            Arc::new(Imports),
            Arc::new(Metadata(id)),
            opener.clone(),
            "https://www.nexusmods.com/monsterhunterworld/mods/",
        ),
        opener,
    )
}

#[test]
fn opens_the_display_revision_package_instead_of_guessing_from_mod_id() {
    let (service, opener) = service(Some(8794));
    service.open_mod_folder("mod-a").unwrap();
    assert_eq!(*opener.0.lock().unwrap(), vec!["display-package"]);
    assert_eq!(
        service.nexus_page_url("mod-a").unwrap(),
        "https://www.nexusmods.com/monsterhunterworld/mods/8794"
    );
}

#[test]
fn rejects_deleted_and_mismatched_mods_without_opening_anything() {
    let (service, opener) = service(Some(8794));
    assert_eq!(
        service.open_mod_folder("missing"),
        Err(ModShortcutError::NotFound)
    );
    assert_eq!(
        service.open_mod_folder("wrong-owner"),
        Err(ModShortcutError::Unavailable)
    );
    assert_eq!(
        service.open_mod_folder(" "),
        Err(ModShortcutError::InvalidMod)
    );
    assert_eq!(
        service.nexus_page_url("missing"),
        Err(ModShortcutError::NotFound)
    );
    assert!(opener.0.lock().unwrap().is_empty());
}

#[test]
fn missing_and_zero_nexus_ids_do_not_produce_a_url() {
    for id in [None, Some(0)] {
        assert_eq!(
            service(id).0.nexus_page_url("mod-a"),
            Err(ModShortcutError::NexusIdMissing)
        );
    }
}

#[test]
fn nexus_id_is_formatted_without_loss_of_integer_precision() {
    assert_eq!(
        service(Some(u64::MAX)).0.nexus_page_url("mod-a").unwrap(),
        "https://www.nexusmods.com/monsterhunterworld/mods/18446744073709551615"
    );
}
