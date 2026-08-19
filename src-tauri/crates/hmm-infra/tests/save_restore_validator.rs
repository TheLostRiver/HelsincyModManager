use hmm_core::{
    GameId, ProfileBackupRetention, ProfileDirectoryMode, ProfileDirectorySelection,
    ProfileDirectoryStatus, ProfileId, SaveBackupManifest, SaveBackupManifestFile,
    SaveBackupManifestSource, SaveBackupStatus, SaveBackupSummary, SaveBackupTrigger,
};
use hmm_infra::{FileSystemSaveBackupWriter, FileSystemSaveRestoreSourceValidator};
use hmm_ports::{
    SaveBackupWriteRequest, SaveBackupWriter, SaveRestoreSourceError, SaveRestoreSourceValidator,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use zip::write::SimpleFileOptions;

#[test]
fn validator_accepts_writer_manifest_and_returns_content_facts() {
    let temp = tempfile::tempdir().expect("temp dir");
    let app_data = temp.path().join("app-data");
    let save_root = temp.path().join("save-root");
    fs::create_dir_all(&save_root).expect("create save root");
    fs::write(save_root.join("SAVEDATA1000"), b"fixture-save").expect("write save");

    let writer = FileSystemSaveBackupWriter::new(app_data.clone());
    let summary = writer
        .write_backup(SaveBackupWriteRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            trigger: SaveBackupTrigger::Manual,
            source_directory: Some(save_root.to_string_lossy().into_owned()),
            source_directory_selection: custom_selection(&save_root),
            backup_directory: default_selection(),
            retention: ProfileBackupRetention::default(),
            note: None,
            created_at_unix_millis: 0,
        })
        .expect("write fixture backup")
        .summary;

    let facts = FileSystemSaveRestoreSourceValidator::new(app_data.clone())
        .validate_source(&summary)
        .expect("validate fixture backup");
    assert_eq!(facts.backup_id, summary.backup_id);
    assert_eq!(facts.file_count, 1);
    assert_eq!(facts.total_uncompressed_bytes, b"fixture-save".len() as u64);
    assert!(facts.evidence_digest.starts_with("sha256:"));
    assert!(
        !app_data.join("save-restore").exists(),
        "preview validation must not create restore staging"
    );
}

#[test]
fn validator_rejects_archive_hash_drift_before_extracting() {
    let (app_data, summary, archive_path) = fixture_backup();
    fs::OpenOptions::new()
        .append(true)
        .open(&archive_path)
        .expect("open archive")
        .write_all(b"tamper")
        .expect("tamper archive");

    let error = FileSystemSaveRestoreSourceValidator::new(app_data)
        .validate_source(&summary)
        .expect_err("tampered archive must fail");
    assert_eq!(error, SaveRestoreSourceError::HashMismatch);
}

#[test]
fn validator_rejects_parent_path_in_manifest_and_zip() {
    let temp = tempfile::tempdir().expect("temp dir");
    let app_data = temp.path().join("app-data");
    let backup_dir = app_data
        .join("saves")
        .join("mhw")
        .join("profile-default");
    fs::create_dir_all(&backup_dir).expect("create backup dir");
    let archive_name = "fixture.zip";
    let manifest_name = "fixture.manifest.json";
    let archive_path = backup_dir.join(archive_name);
    let mut zip = zip::ZipWriter::new(fs::File::create(&archive_path).expect("create zip"));
    zip.start_file("../escape", SimpleFileOptions::default())
        .expect("add unsafe entry");
    zip.write_all(b"unsafe").expect("write unsafe entry");
    zip.finish().expect("finish zip");

    let archive_bytes = fs::read(&archive_path).expect("read zip");
    let manifest = SaveBackupManifest {
        schema_version: 1,
        backup_id: "fixture".to_owned(),
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        trigger: SaveBackupTrigger::Manual,
        created_at_utc: "1970-01-01T00:00:00Z".to_owned(),
        created_at_utc_label: "1970-01-01 00:00:00 UTC".to_owned(),
        archive_file_name: archive_name.to_owned(),
        archive_size_bytes: archive_bytes.len() as u64,
        archive_sha256: sha256(&archive_bytes),
        source: SaveBackupManifestSource {
            mode: "custom".to_owned(),
            path_label: Some("fixture".to_owned()),
            path_hash: "sha256:source".to_owned(),
        },
        files: vec![SaveBackupManifestFile {
            relative_path: "../escape".to_owned(),
            size_bytes: 6,
            sha256: sha256(b"unsafe"),
            modified_at_utc: None,
        }],
        notes: None,
    };
    fs::write(
        backup_dir.join(manifest_name),
        serde_json::to_vec(&manifest).expect("manifest json"),
    )
    .expect("write manifest");

    let summary = SaveBackupSummary {
        backup_id: "fixture".to_owned(),
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        trigger: SaveBackupTrigger::Manual,
        status: SaveBackupStatus::Completed,
        archive_file_name: archive_name.to_owned(),
        manifest_file_name: manifest_name.to_owned(),
        archive_size_bytes: archive_bytes.len() as u64,
        retention_released_bytes: 0,
        archive_sha256: sha256(&archive_bytes),
        file_count: 1,
        created_at: 0,
        source_path_label: Some("fixture".to_owned()),
        source_path_hash: "sha256:source".to_owned(),
        backup_directory: default_selection(),
        notes: None,
    };

    let error = FileSystemSaveRestoreSourceValidator::new(app_data)
        .validate_source(&summary)
        .expect_err("unsafe path must fail");
    assert_eq!(error, SaveRestoreSourceError::UnsafePath);
}

fn fixture_backup() -> (std::path::PathBuf, SaveBackupSummary, std::path::PathBuf) {
    let temp = tempfile::tempdir().expect("temp dir");
    let app_data = temp.path().join("app-data");
    let save_root = temp.path().join("save-root");
    fs::create_dir_all(&save_root).expect("create save root");
    fs::write(save_root.join("SAVEDATA1000"), b"fixture-save").expect("write save");
    let summary = FileSystemSaveBackupWriter::new(app_data.clone())
        .write_backup(SaveBackupWriteRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            trigger: SaveBackupTrigger::Manual,
            source_directory: Some(save_root.to_string_lossy().into_owned()),
            source_directory_selection: custom_selection(&save_root),
            backup_directory: default_selection(),
            retention: ProfileBackupRetention::default(),
            note: None,
            created_at_unix_millis: 0,
        })
        .expect("write fixture backup")
        .summary;
    let archive_path = app_data
        .join("saves")
        .join("mhw")
        .join("profile-default")
        .join(&summary.archive_file_name);
    std::mem::forget(temp);
    (app_data, summary, archive_path)
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

fn default_selection() -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Default,
        status: ProfileDirectoryStatus::Defaulted,
        directory: None,
        path_label: Some("文档/HelsincyModManager/saves".to_owned()),
        messages: Vec::new(),
    }
}

fn custom_selection(path: &std::path::Path) -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Custom,
        status: ProfileDirectoryStatus::Valid,
        directory: Some(path.to_string_lossy().into_owned()),
        path_label: Some("fixture".to_owned()),
        messages: Vec::new(),
    }
}
