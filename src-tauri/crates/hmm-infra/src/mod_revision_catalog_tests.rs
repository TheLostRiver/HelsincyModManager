use crate::mod_revision_catalog::{JsonModImportResultRepository, ModImportCatalogWriteFailure};
use hmm_core::{ModId, ModRevisionId, PreviewImageRejectionReason};
use hmm_ports::{
    ModImportResultRepository, StoredImportPreviewImage, StoredLogicalMod, StoredModImportAnalysis,
    StoredModOriginProvenance, StoredModPackageMetadata, StoredModRevision,
};
use serde_json::Value;
use std::fs;
use std::path::Path;

#[test]
fn mod_import_catalog_migrates_v1_record_without_losing_identity_or_provenance() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("results.json");
    write_v1_catalog(&path);
    let repo = JsonModImportResultRepository::new(path.clone());

    let mods = repo.list_mods().expect("migrate and list logical Mods");
    let revisions = repo
        .list_revisions(&ModId::new("legacy-mod"))
        .expect("list migrated revisions");

    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].mod_id, ModId::new("legacy-mod"));
    assert_eq!(
        mods[0].origin_revision_id,
        ModRevisionId::new("legacy-package")
    );
    assert_eq!(
        mods[0].display_revision_id,
        ModRevisionId::new("legacy-package")
    );
    assert_eq!(
        mods[0].origin_provenance,
        StoredModOriginProvenance::MigratedV1 {
            legacy_mod_id: "legacy-mod".to_owned(),
            legacy_package_id: "legacy-package".to_owned(),
        }
    );
    assert_eq!(revisions.len(), 1);
    assert_eq!(
        revisions[0].revision_id,
        ModRevisionId::new("legacy-package")
    );
    assert_eq!(revisions[0].mod_id, ModId::new("legacy-mod"));
    assert_eq!(revisions[0].package_id, "legacy-package");
    assert_eq!(revisions[0].import_task_id, "legacy-task");
    assert_eq!(revisions[0].display_name, "Legacy Mod");
    assert_eq!(revisions[0].metadata.version.as_deref(), Some("1.0"));
    assert_eq!(revisions[0].metadata.category.as_deref(), Some("armor"));

    let persisted = read_json(&path);
    assert_eq!(persisted["version"], 2);
    assert_eq!(persisted["mods"].as_array().map(Vec::len), Some(1));
    assert_eq!(persisted["revisions"].as_array().map(Vec::len), Some(1));
    assert!(persisted.get("records").is_none());
}

#[test]
fn mod_import_catalog_append_preserves_origin_and_projects_one_display_card_after_reload() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("results.json");
    write_v1_catalog(&path);
    let repo = JsonModImportResultRepository::new(path.clone());

    repo.append_revision(&revision(
        "revision-v2",
        "legacy-mod",
        "package-v2",
        "task-v2",
    ))
    .expect("append candidate revision");

    let logical_mod = repo
        .get_mod(&ModId::new("legacy-mod"))
        .expect("read logical Mod")
        .expect("logical Mod exists");
    assert_eq!(
        logical_mod.origin_revision_id,
        ModRevisionId::new("legacy-package")
    );
    assert_eq!(
        logical_mod.display_revision_id,
        ModRevisionId::new("revision-v2")
    );

    let reloaded = JsonModImportResultRepository::new(path);
    let cards = reloaded.list_analysis().expect("list compatibility cards");
    let revisions = reloaded
        .list_revisions(&ModId::new("legacy-mod"))
        .expect("list revisions after reload");

    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].mod_id, "legacy-mod");
    assert_eq!(cards[0].package_id, "package-v2");
    assert_eq!(revisions.len(), 2);
    assert_eq!(
        revisions[0].revision_id,
        ModRevisionId::new("legacy-package")
    );
    assert_eq!(revisions[1].revision_id, ModRevisionId::new("revision-v2"));
}

#[test]
fn mod_import_catalog_rejects_revision_rebinding_across_logical_mods() {
    let temp = tempfile::tempdir().expect("temp dir");
    let repo = JsonModImportResultRepository::new(temp.path().join("results.json"));

    repo.save_new_mod(
        &logical_mod("mod-a", "shared-revision"),
        &revision("shared-revision", "mod-a", "package-a", "task-a"),
    )
    .expect("save Mod A");
    repo.save_new_mod(
        &logical_mod("mod-b", "revision-b"),
        &revision("revision-b", "mod-b", "package-b", "task-b"),
    )
    .expect("save Mod B");

    let error = repo
        .append_revision(&revision("shared-revision", "mod-b", "package-c", "task-c"))
        .expect_err("revision cannot be rebound to another Mod");

    assert!(error
        .to_string()
        .contains("already belongs to another logical Mod"));
    let stored = repo
        .get_revision(&ModRevisionId::new("shared-revision"))
        .expect("read original revision")
        .expect("original revision exists");
    assert_eq!(stored.mod_id, ModId::new("mod-a"));
}

#[test]
fn mod_import_catalog_migration_write_failures_leave_original_v1_bytes_intact() {
    for failure in [
        ModImportCatalogWriteFailure::TempWrite,
        ModImportCatalogWriteFailure::Rename,
    ] {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("results.json");
        write_v1_catalog(&path);
        let original = fs::read(&path).expect("read original v1 bytes");
        let repo =
            JsonModImportResultRepository::new(path.clone()).with_test_write_failure(failure);

        repo.list_revisions(&ModId::new("legacy-mod"))
            .expect_err("injected migration write failure");

        assert_eq!(fs::read(&path).expect("read v1 after failure"), original);
        assert!(!fs::read_dir(temp.path())
            .expect("list catalog directory")
            .filter_map(Result::ok)
            .any(|entry| entry.path().extension().is_some_and(|ext| ext == "tmp")));
        let recovered = JsonModImportResultRepository::new(path);
        assert_eq!(
            recovered
                .list_revisions(&ModId::new("legacy-mod"))
                .expect("retry migration")
                .len(),
            1
        );
    }
}

#[test]
fn mod_import_catalog_unlock_failure_does_not_override_durable_success() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("results.json");
    let repo = JsonModImportResultRepository::new(path.clone())
        .with_test_write_failure(ModImportCatalogWriteFailure::Unlock);

    repo.save_new_mod(
        &logical_mod("mod-a", "revision-a"),
        &revision("revision-a", "mod-a", "package-a", "task-a"),
    )
    .expect("durable mutation stays successful when explicit unlock fails");

    let reloaded = JsonModImportResultRepository::new(path);
    assert!(reloaded
        .get_mod(&ModId::new("mod-a"))
        .expect("reload catalog")
        .is_some());
}

#[test]
fn mod_import_catalog_missing_store_loads_an_empty_library() {
    let temp = tempfile::tempdir().expect("temp dir");
    let repo = JsonModImportResultRepository::new(temp.path().join("results.json"));

    assert!(repo.list_analysis().expect("list analysis").is_empty());
    assert!(repo.list_mods().expect("list logical Mods").is_empty());
}

#[test]
fn mod_import_catalog_compatibility_save_updates_preview_without_appending() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("results.json");
    let repo = JsonModImportResultRepository::new(path.clone());
    let mut analysis = analysis("mod-a", "package-a", PreviewImageRejectionReason::Missing);
    repo.save_analysis(&analysis)
        .expect("save compatibility record");

    analysis.preview_image = StoredImportPreviewImage::Fallback {
        reason: PreviewImageRejectionReason::DecodeFailed,
    };
    analysis.display_name = "Attempted Mutation".to_owned();
    repo.save_analysis(&analysis)
        .expect("update compatibility preview");

    let cards = repo.list_analysis().expect("list cards");
    let revisions = repo
        .list_revisions(&ModId::new("mod-a"))
        .expect("list revisions");
    assert_eq!(cards.len(), 1);
    assert_eq!(revisions.len(), 1);
    assert_eq!(cards[0].display_name, "Original Mod");
    assert_eq!(
        cards[0].preview_image,
        StoredImportPreviewImage::Fallback {
            reason: PreviewImageRejectionReason::DecodeFailed,
        }
    );
    assert_eq!(read_json(&path)["version"], 2);
}

#[test]
fn mod_import_catalog_migration_preserves_legacy_optional_field_defaults() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("results.json");
    fs::write(
        &path,
        r#"{
  "version": 1,
  "records": [{
    "mod_id": "legacy-mod",
    "task_id": "legacy-task",
    "package_id": "legacy-package",
    "display_name": "Legacy Mod"
  }]
}"#,
    )
    .expect("write minimal v1 catalog");
    let repo = JsonModImportResultRepository::new(path);

    let card = repo
        .get_analysis("legacy-mod")
        .expect("migrate legacy record")
        .expect("legacy card exists");

    assert_eq!(card.metadata, StoredModPackageMetadata::default());
    assert_eq!(
        card.preview_image,
        StoredImportPreviewImage::Fallback {
            reason: PreviewImageRejectionReason::Missing,
        }
    );
}

#[test]
fn mod_import_catalog_rejects_new_mod_owner_mismatch() {
    let temp = tempfile::tempdir().expect("temp dir");
    let repo = JsonModImportResultRepository::new(temp.path().join("results.json"));

    let error = repo
        .save_new_mod(
            &logical_mod("mod-a", "revision-a"),
            &revision("revision-a", "mod-b", "package-a", "task-a"),
        )
        .expect_err("origin revision owner mismatch rejected");

    assert!(error
        .to_string()
        .contains("logical Mod and origin revision do not match"));
    assert!(repo.list_mods().expect("list logical Mods").is_empty());
}

#[test]
fn mod_import_catalog_rejects_duplicate_package_for_distinct_revisions() {
    let temp = tempfile::tempdir().expect("temp dir");
    let repo = JsonModImportResultRepository::new(temp.path().join("results.json"));
    repo.save_new_mod(
        &logical_mod("mod-a", "revision-a"),
        &revision("revision-a", "mod-a", "shared-package", "task-a"),
    )
    .expect("save Mod A");
    repo.save_new_mod(
        &logical_mod("mod-b", "revision-b"),
        &revision("revision-b", "mod-b", "package-b", "task-b"),
    )
    .expect("save Mod B");

    let error = repo
        .append_revision(&revision(
            "revision-c",
            "mod-b",
            "shared-package",
            "task-c",
        ))
        .expect_err("package identity cannot be reused by another revision");

    assert!(error
        .to_string()
        .contains("package already belongs to another revision"));
    assert_eq!(
        repo.list_revisions(&ModId::new("mod-b"))
            .expect("list Mod B revisions")
            .len(),
        1
    );
}

#[test]
fn mod_import_catalog_rejects_tampered_v1_migration_provenance() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("results.json");
    fs::write(
        &path,
        r#"{
  "version": 2,
  "mods": [{
    "mod_id": "mod-a",
    "origin_revision_id": "revision-a",
    "display_revision_id": "revision-a",
    "origin_provenance": {
      "kind": "migrated_v1",
      "legacy_mod_id": "other-mod",
      "legacy_package_id": "other-package"
    }
  }],
  "revisions": [{
    "revision_id": "revision-a",
    "mod_id": "mod-a",
    "import_task_id": "task-a",
    "package_id": "package-a",
    "display_name": "Mod A"
  }]
}"#,
    )
    .expect("write tampered v2 catalog");
    let repo = JsonModImportResultRepository::new(path);

    let error = repo
        .list_mods()
        .expect_err("tampered migration provenance rejected");

    assert!(error
        .to_string()
        .contains("migration provenance does not match origin revision"));
}

fn logical_mod(mod_id: &str, revision_id: &str) -> StoredLogicalMod {
    let revision_id = ModRevisionId::new(revision_id);
    StoredLogicalMod {
        mod_id: ModId::new(mod_id),
        origin_revision_id: revision_id.clone(),
        display_revision_id: revision_id,
        origin_provenance: StoredModOriginProvenance::Imported,
    }
}

fn revision(revision_id: &str, mod_id: &str, package_id: &str, task_id: &str) -> StoredModRevision {
    StoredModRevision {
        revision_id: ModRevisionId::new(revision_id),
        mod_id: ModId::new(mod_id),
        import_task_id: task_id.to_owned(),
        package_id: package_id.to_owned(),
        display_name: format!("Revision {revision_id}"),
        metadata: StoredModPackageMetadata::default(),
        preview_image: StoredImportPreviewImage::Fallback {
            reason: PreviewImageRejectionReason::Missing,
        },
    }
}

fn analysis(
    mod_id: &str,
    package_id: &str,
    reason: PreviewImageRejectionReason,
) -> StoredModImportAnalysis {
    StoredModImportAnalysis {
        mod_id: mod_id.to_owned(),
        task_id: "task-a".to_owned(),
        package_id: package_id.to_owned(),
        display_name: "Original Mod".to_owned(),
        metadata: StoredModPackageMetadata::default(),
        preview_image: StoredImportPreviewImage::Fallback { reason },
    }
}

fn write_v1_catalog(path: &Path) {
    fs::write(
        path,
        r#"{
  "version": 1,
  "records": [
    {
      "mod_id": "legacy-mod",
      "task_id": "legacy-task",
      "package_id": "legacy-package",
      "display_name": "Legacy Mod",
      "metadata": {
        "version": "1.0",
        "author": "Fixture Author",
        "category": "armor",
        "tags": ["fixture"],
        "dependencies": ["loader"]
      },
      "preview_image": {
        "kind": "fallback",
        "reason": "missing"
      }
    }
  ]
}"#,
    )
    .expect("write v1 catalog");
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).expect("read catalog")).expect("parse catalog JSON")
}
