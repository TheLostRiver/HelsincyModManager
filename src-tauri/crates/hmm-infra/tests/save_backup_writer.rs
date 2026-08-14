use hmm_core::{
    GameId, ProfileBackupRetention, ProfileDirectoryMode, ProfileDirectorySelection,
    ProfileDirectoryStatus, ProfileId, SaveBackupManifest, SaveBackupTrigger,
};
use hmm_infra::FileSystemSaveBackupWriter;
use hmm_ports::{SaveBackupWriteRequest, SaveBackupWriter};
use std::fs;
use zip::ZipArchive;

#[test]
fn file_system_save_backup_writer_creates_zip_manifest_and_summary_without_raw_source_path() {
    let temp = tempfile::tempdir().expect("temp dir");
    let app_data = temp.path().join("app-data");
    let save_root = temp.path().join("steam-user").join("582010").join("remote");
    fs::create_dir_all(&save_root).expect("create save root");
    fs::write(save_root.join("SAVEDATA1000"), b"hunter-save").expect("write save");
    fs::create_dir_all(save_root.join("nested")).expect("create nested");
    fs::write(save_root.join("nested").join("config.dat"), b"config").expect("write nested");

    let writer = FileSystemSaveBackupWriter::new(app_data.clone());
    let result = writer
        .write_backup(SaveBackupWriteRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            trigger: SaveBackupTrigger::Manual,
            source_directory: Some(save_root.to_string_lossy().into_owned()),
            source_directory_selection: custom_source_directory_selection(&save_root),
            backup_directory: default_backup_directory_selection(),
            retention: ProfileBackupRetention {
                max_count: 20,
                max_age_days: None,
            },
            note: Some("manual smoke".to_owned()),
            created_at_unix_millis: 0,
        })
        .expect("backup should be written");

    assert_eq!(result.summary.game_id.as_str(), "mhw");
    assert_eq!(result.summary.profile_id.as_str(), "default");
    assert_eq!(result.summary.trigger, SaveBackupTrigger::Manual);
    assert_eq!(result.summary.file_count, 2);
    assert!(result.summary.archive_file_name.ends_with("_manual.zip"));
    assert!(result
        .summary
        .manifest_file_name
        .ends_with("_manual.manifest.json"));

    let backup_dir = app_data
        .join("backups")
        .join("saves")
        .join("mhw")
        .join("profile-default");
    let archive_path = backup_dir.join(&result.summary.archive_file_name);
    let manifest_path = backup_dir.join(&result.summary.manifest_file_name);
    assert!(archive_path.exists());
    assert!(manifest_path.exists());

    let archive_file = fs::File::open(&archive_path).expect("open archive");
    let mut archive = ZipArchive::new(archive_file).expect("read archive");
    let mut entry_names = (0..archive.len())
        .map(|index| {
            archive
                .by_index(index)
                .expect("zip entry")
                .name()
                .to_owned()
        })
        .collect::<Vec<_>>();
    entry_names.sort();
    assert_eq!(entry_names, vec!["SAVEDATA1000", "nested/config.dat"]);

    let manifest_text = fs::read_to_string(&manifest_path).expect("read manifest");
    let manifest: SaveBackupManifest = serde_json::from_str(&manifest_text).expect("manifest json");
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.game_id.as_str(), "mhw");
    assert_eq!(manifest.profile_id.as_str(), "default");
    assert_eq!(manifest.archive_file_name, result.summary.archive_file_name);
    assert_eq!(manifest.source.mode, "custom");
    assert_eq!(manifest.files.len(), 2);
    assert!(manifest
        .files
        .iter()
        .any(|file| file.relative_path == "SAVEDATA1000"));
    assert!(!manifest_text.contains(save_root.to_string_lossy().as_ref()));
    assert!(!manifest_text.contains("steam-user"));
    assert!(!manifest_text.contains("582010"));
}

#[test]
fn file_system_save_backup_writer_keeps_same_second_backups_as_distinct_history_facts() {
    let temp = tempfile::tempdir().expect("temp dir");
    let app_data = temp.path().join("app-data");
    let save_root = temp.path().join("save-root");
    fs::create_dir_all(&save_root).expect("create save root");
    fs::write(save_root.join("SAVEDATA1000"), b"hunter-save").expect("write save");

    let writer = FileSystemSaveBackupWriter::new(app_data);
    let first = writer
        .write_backup(sample_write_request(&save_root, 0))
        .expect("first backup should be written");
    let second = writer
        .write_backup(sample_write_request(&save_root, 0))
        .expect("same-second backup should be written with sequence");

    assert_ne!(
        first.summary.archive_file_name,
        second.summary.archive_file_name
    );
    assert!(second.summary.archive_file_name.contains("_manual_02.zip"));
    assert_ne!(first.summary.backup_id, second.summary.backup_id);
    assert!(
        second.summary.backup_id.ends_with(":02"),
        "second backup id should include the same-second sequence"
    );
}

#[test]
fn file_system_save_backup_writer_places_custom_roots_under_managed_profile_folder() {
    let temp = tempfile::tempdir().expect("temp dir");
    let app_data = temp.path().join("app-data");
    let save_root = temp.path().join("save-root");
    let chosen_root = temp.path().join("chosen-backups");
    fs::create_dir_all(&save_root).expect("create save root");
    fs::create_dir_all(&chosen_root).expect("create chosen root");
    fs::write(save_root.join("SAVEDATA1000"), b"hunter-save").expect("write save");

    let writer = FileSystemSaveBackupWriter::new(app_data);
    let mut request = sample_write_request(&save_root, 0);
    request.backup_directory = custom_backup_directory_selection(&chosen_root);

    let result = writer
        .write_backup(request)
        .expect("custom backup root should be managed");

    let managed_dir = chosen_root
        .join("HelsincyModManager")
        .join("saves")
        .join("mhw")
        .join("profile-default");
    assert!(managed_dir.join(result.summary.archive_file_name).exists());
    assert!(managed_dir.join(result.summary.manifest_file_name).exists());
}

#[test]
fn file_system_save_backup_writer_places_pre_restore_backups_in_dedicated_folder() {
    let temp = tempfile::tempdir().expect("temp dir");
    let app_data = temp.path().join("app-data");
    let save_root = temp.path().join("save-root");
    fs::create_dir_all(&save_root).expect("create save root");
    fs::write(save_root.join("SAVEDATA1000"), b"before-restore").expect("write save");

    let writer = FileSystemSaveBackupWriter::new(app_data.clone());
    let mut request = sample_write_request(&save_root, 0);
    request.trigger = SaveBackupTrigger::PreRestore;
    let result = writer
        .write_backup(request)
        .expect("pre-restore backup should be written");

    let directory = app_data
        .join("backups")
        .join("saves")
        .join("mhw")
        .join("profile-default")
        .join("pre-restore");
    assert!(directory.join(&result.summary.archive_file_name).exists());
    assert!(directory.join(&result.summary.manifest_file_name).exists());
    assert!(result
        .summary
        .archive_file_name
        .ends_with("_pre_restore.zip"));
}

#[test]
fn file_system_save_backup_writer_rejects_destination_inside_source() {
    let temp = tempfile::tempdir().expect("temp dir");
    let save_root = temp.path().join("save-root");
    fs::create_dir_all(&save_root).expect("create save root");
    fs::write(save_root.join("SAVEDATA1000"), b"hunter-save").expect("write save");

    let app_data_inside_source = save_root.join("app-data");
    let writer = FileSystemSaveBackupWriter::new(app_data_inside_source);

    let error = writer
        .write_backup(sample_write_request(&save_root, 0))
        .expect_err("destination inside source should be rejected");

    assert!(error.to_string().contains("destination"));
}

#[test]
fn file_system_save_backup_writer_rejects_linked_directory_without_archiving_outside_files() {
    let temp = tempfile::tempdir().expect("temp dir");
    let app_data = temp.path().join("app-data");
    let save_root = temp.path().join("save-root");
    let outside_root = temp.path().join("outside-root");
    fs::create_dir_all(&save_root).expect("create save root");
    fs::create_dir_all(&outside_root).expect("create outside root");
    fs::write(save_root.join("SAVEDATA1000"), b"hunter-save").expect("write save");
    fs::write(
        outside_root.join("outside.sentinel"),
        b"must-not-be-archived",
    )
    .expect("write outside sentinel");

    let linked_directory = save_root.join("linked");
    create_directory_link(&outside_root, &linked_directory);

    let writer = FileSystemSaveBackupWriter::new(app_data.clone());
    let error = writer
        .write_backup(sample_write_request(&save_root, 0))
        .expect_err("save backup must reject linked directories");
    assert!(error.to_string().contains("link"));
    assert_eq!(
        fs::read(outside_root.join("outside.sentinel")).expect("read outside sentinel"),
        b"must-not-be-archived"
    );
    assert!(
        !app_data.exists()
            || fs::read_dir(&app_data)
                .expect("read app data")
                .next()
                .is_none(),
        "rejected backup must not leave an archive"
    );
    remove_directory_link(&linked_directory);
}

#[test]
fn file_system_save_backup_writer_rejects_linked_source_root() {
    let temp = tempfile::tempdir().expect("temp dir");
    let app_data = temp.path().join("app-data");
    let real_root = temp.path().join("real-root");
    let linked_root = temp.path().join("linked-root");
    fs::create_dir_all(&real_root).expect("create real root");
    fs::write(real_root.join("SAVEDATA1000"), b"hunter-save").expect("write save");
    create_directory_link(&real_root, &linked_root);

    let writer = FileSystemSaveBackupWriter::new(app_data);
    let error = writer
        .write_backup(sample_write_request(&linked_root, 0))
        .expect_err("linked source root must be rejected");
    assert!(error.to_string().contains("link"));
    remove_directory_link(&linked_root);
}

#[cfg(unix)]
fn create_directory_link(target: &std::path::Path, link: &std::path::Path) {
    std::os::unix::fs::symlink(target, link).expect("create directory symlink");
}

#[cfg(windows)]
fn create_directory_link(target: &std::path::Path, link: &std::path::Path) {
    let output = std::process::Command::new("cmd")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .expect("create directory junction");
    assert!(
        output.status.success(),
        "mklink failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
fn remove_directory_link(link: &std::path::Path) {
    fs::remove_file(link).expect("remove directory symlink");
}

#[cfg(windows)]
fn remove_directory_link(link: &std::path::Path) {
    fs::remove_dir(link).expect("remove directory junction");
}

fn sample_write_request(
    save_root: &std::path::Path,
    created_at_unix_millis: u128,
) -> SaveBackupWriteRequest {
    SaveBackupWriteRequest {
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        trigger: SaveBackupTrigger::Manual,
        source_directory: Some(save_root.to_string_lossy().into_owned()),
        source_directory_selection: custom_source_directory_selection(save_root),
        backup_directory: default_backup_directory_selection(),
        retention: ProfileBackupRetention {
            max_count: 20,
            max_age_days: None,
        },
        note: None,
        created_at_unix_millis,
    }
}

fn default_backup_directory_selection() -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Default,
        status: ProfileDirectoryStatus::Defaulted,
        directory: None,
        path_label: Some("HelsincyModManager/backups/saves/mhw/profile-default".to_owned()),
        messages: Vec::new(),
    }
}

fn custom_backup_directory_selection(root: &std::path::Path) -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Custom,
        status: ProfileDirectoryStatus::Valid,
        directory: Some(root.to_string_lossy().into_owned()),
        path_label: Some("chosen-backups".to_owned()),
        messages: Vec::new(),
    }
}

fn custom_source_directory_selection(root: &std::path::Path) -> ProfileDirectorySelection {
    ProfileDirectorySelection {
        mode: ProfileDirectoryMode::Custom,
        status: ProfileDirectoryStatus::Valid,
        directory: Some(root.to_string_lossy().into_owned()),
        path_label: Some("save-root".to_owned()),
        messages: Vec::new(),
    }
}
