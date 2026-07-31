use hmm_core::{
    InstallRecoveryRecord, InstallRecoveryRecordEntry, InstallRecoveryRecordStatus,
    InstallTargetPath, InstalledFileSummary, ModId, ModRevisionId, PackageFileId,
    PreviewImageRejectionReason, ProfileId,
};
use hmm_infra::{
    JsonInstallManifestRepository, JsonInstallRecoveryRecordRepository,
    JsonModImportResultRepository,
};
use hmm_ports::{
    InstallManifestRepository, InstallRecoveryRecordRepository, ModImportResultRepository,
    StoredImportPreviewImage, StoredModPackageMetadata, StoredModRevision,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn hmm(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_hmm"))
        .args(args)
        .output()
        .expect("run hmm")
}

fn stdout_text(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is utf-8")
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is utf-8")
}

fn absolute_sandbox_path() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("cli-contract-sandbox")
        .leak()
}

fn hmm_in_sandbox(sandbox: &Path, format: &str, command: &[&str]) -> Output {
    hmm_group_in_sandbox(sandbox, format, "game", command)
}

fn hmm_install_in_sandbox(sandbox: &Path, format: &str, command: &[&str]) -> Output {
    hmm_group_in_sandbox(sandbox, format, "install", command)
}

fn hmm_backup_in_sandbox(sandbox: &Path, format: &str, command: &[&str]) -> Output {
    hmm_group_in_sandbox(sandbox, format, "backup", command)
}

fn hmm_diagnostics_in_sandbox(sandbox: &Path, format: &str, command: &[&str]) -> Output {
    hmm_group_in_sandbox(sandbox, format, "diagnostics", command)
}

fn hmm_group_in_sandbox(sandbox: &Path, format: &str, group: &str, command: &[&str]) -> Output {
    let mut process = Command::new(env!("CARGO_BIN_EXE_hmm"));
    process
        .arg("--format")
        .arg(format)
        .arg("--environment")
        .arg("sandbox")
        .arg("--data-dir")
        .arg(sandbox)
        .arg(group)
        .args(command);
    process.output().expect("run sandbox hmm")
}

fn create_game_fixture(sandbox: &Path, include_executable: bool) -> PathBuf {
    let game_root = sandbox.join("fixtures").join("games").join("mhw-minimal");
    fs::create_dir_all(&game_root).expect("create game fixture");
    if include_executable {
        fs::write(game_root.join("MonsterHunterWorld.exe"), b"fixture").expect("write game exe");
    }
    game_root
}

fn write_unverified_prerequisite_fixture(game_root: &Path) {
    fs::create_dir_all(game_root.join("nativePC/plugins"))
        .expect("create prerequisite fixture directory");
    for relative_path in [
        "dinput8.dll",
        "loader.dll",
        "nativePC/plugins/MonsterLoader.dll",
        "nativePC/plugins/QuestLoader.dll",
        "nativePC/plugins/!CRCBypass.dll",
    ] {
        fs::write(game_root.join(relative_path), b"artificial-prerequisite")
            .expect("write prerequisite fixture");
    }
    fs::write(
        game_root.join("loader-config.json"),
        br#"{"enablePluginLoader":true}"#,
    )
    .expect("write prerequisite config");
}

fn write_game_config(sandbox: &Path, game_root: &Path) {
    let config_root = sandbox.join("config");
    fs::create_dir_all(&config_root).expect("create config root");
    fs::write(
        config_root.join("games.json"),
        serde_json::json!({
            "version": 1,
            "games": [{
                "id": "mhw-default",
                "game_id": "mhw",
                "display_name": "Monster Hunter: World - Iceborne",
                "root_dir": game_root,
                "status": "configured",
                "configured_at_unix_millis": 42
            }]
        })
        .to_string(),
    )
    .expect("write game config");
}

fn write_sandbox_marker(sandbox: &Path) {
    fs::write(
        sandbox.join(hmm_runtime::SANDBOX_MARKER_FILE_NAME),
        hmm_runtime::SANDBOX_MARKER_SCHEMA,
    )
    .expect("write sandbox marker");
}

fn write_mod_catalog_and_sandbox(sandbox: &Path) {
    let catalog_root = sandbox.join("mod-import");
    fs::create_dir_all(&catalog_root).expect("create catalog root");
    fs::write(
        catalog_root.join("results.json"),
        r#"{
  "version": 1,
  "records": [{
    "mod_id": "mod-a",
    "task_id": "task-a",
    "package_id": "package-a",
    "display_name": "Fixture Mod"
  }]
}"#,
    )
    .expect("write Mod catalog");
    let package_root = catalog_root
        .join("sandboxes")
        .join("package-a")
        .join("nativePC")
        .join("models");
    fs::create_dir_all(&package_root).expect("create package root");
    fs::write(package_root.join("player.mod3"), b"fixture").expect("write package fixture");
}

fn write_reinstall_v1_catalog_and_sandbox(sandbox: &Path) {
    let catalog_root = sandbox.join("mod-import");
    fs::create_dir_all(&catalog_root).expect("create reinstall catalog root");
    fs::write(
        catalog_root.join("results.json"),
        r#"{
  "version": 1,
  "records": [{
    "mod_id": "mod-a",
    "task_id": "task-v1",
    "package_id": "package-v1",
    "display_name": "Reinstall Fixture Mod"
  }]
}"#,
    )
    .expect("write reinstall Mod catalog");
    let package_root = catalog_root.join("sandboxes").join("package-v1");
    let files: &[(&str, &[u8])] = &[
        ("nativePC/models/retained.mod3", b"same"),
        ("nativePC/models/replaced.mod3", b"revision-v1"),
        ("nativePC/models/stale.mod3", b"revision-v1-stale"),
    ];
    for &(target, bytes) in files {
        let path = package_root.join(target);
        fs::create_dir_all(path.parent().expect("v1 package target parent"))
            .expect("create v1 package parent");
        fs::write(path, bytes).expect("write v1 package file");
    }
}

fn append_reinstall_v2_revision(sandbox: &Path) {
    let package_root = sandbox
        .join("mod-import")
        .join("sandboxes")
        .join("package-v2");
    let files: &[(&str, &[u8])] = &[
        ("nativePC/models/retained.mod3", b"same"),
        ("nativePC/models/replaced.mod3", b"revision-v2"),
        ("nativePC/models/added.mod3", b"revision-v2-added"),
    ];
    for &(target, bytes) in files {
        let path = package_root.join(target);
        fs::create_dir_all(path.parent().expect("v2 package target parent"))
            .expect("create v2 package parent");
        fs::write(path, bytes).expect("write v2 package file");
    }

    JsonModImportResultRepository::new(sandbox.join("mod-import").join("results.json"))
        .append_revision(&StoredModRevision {
            revision_id: ModRevisionId::new("revision-v2"),
            mod_id: ModId::new("mod-a"),
            import_task_id: "task-v2".to_owned(),
            package_id: "package-v2".to_owned(),
            display_name: "Reinstall Fixture Mod v2".to_owned(),
            metadata: StoredModPackageMetadata::default(),
            preview_image: StoredImportPreviewImage::Fallback {
                reason: PreviewImageRejectionReason::Missing,
            },
        })
        .expect("append v2 revision through catalog repository");
}

fn write_rollback_recovery_fixture(sandbox: &Path, game_root: &Path) -> PathBuf {
    const CONTENT: &[u8] = b"recovery-fixture";
    const SHA256: &str = "f1889dda90864358c71d55bdf593bf568d7bde025635c248182721d319a2aeaf";

    let relative_target = "nativePC/models/recovery.mod3";
    let target = game_root.join(relative_target);
    fs::create_dir_all(target.parent().expect("recovery target parent"))
        .expect("create recovery target parent");
    fs::write(&target, CONTENT).expect("write recovery target");
    let repository =
        JsonInstallRecoveryRecordRepository::new(sandbox.join("install").join("recovery"));
    repository
        .save_record(&InstallRecoveryRecord {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-recovery"),
            status: InstallRecoveryRecordStatus::RollbackRequired,
            entries: vec![InstallRecoveryRecordEntry {
                target_path: InstallTargetPath::parse(relative_target, ["nativePC"])
                    .expect("recovery target path"),
                package_file_id: PackageFileId::new(relative_target),
                backup_ref: None,
                installed_file: Some(InstalledFileSummary {
                    size_bytes: CONTENT.len() as u64,
                    sha256: SHA256.to_owned(),
                }),
            }],
        })
        .expect("write recovery record");
    target
}

fn write_steam_fixture(sandbox: &Path) {
    let steam_root = sandbox.join("fixtures").join("steam");
    let game_root = steam_root
        .join("steamapps")
        .join("common")
        .join("Monster Hunter World");
    fs::create_dir_all(&game_root).expect("create Steam game fixture");
    fs::write(game_root.join("MonsterHunterWorld.exe"), b"fixture").expect("write Steam game exe");
    fs::write(
        steam_root.join("steamapps").join("libraryfolders.vdf"),
        format!(
            r#""libraryfolders" {{ "0" {{ "path" "{}" "apps" {{ "582010" "1" }} }} }}"#,
            steam_root.display()
        ),
    )
    .expect("write library folders");
    fs::write(
        steam_root.join("steamapps").join("appmanifest_582010.acf"),
        r#""AppState" { "appid" "582010" "installdir" "Monster Hunter World" }"#,
    )
    .expect("write app manifest");
}

fn write_backup_database(sandbox: &Path) {
    let connection =
        rusqlite::Connection::open(sandbox.join("hmm.db")).expect("create backup database");
    connection
        .execute_batch(
            "
            CREATE TABLE save_backups (
                backup_id TEXT PRIMARY KEY,
                game_id TEXT NOT NULL,
                profile_id TEXT NOT NULL,
                trigger TEXT NOT NULL,
                status TEXT NOT NULL,
                archive_file_name TEXT NOT NULL,
                manifest_file_name TEXT NOT NULL,
                archive_size_bytes INTEGER NOT NULL,
                archive_sha256 TEXT NOT NULL,
                file_count INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                source_path_label TEXT,
                source_path_hash TEXT NOT NULL,
                notes TEXT,
                backup_directory_mode TEXT NOT NULL,
                backup_directory TEXT
            );
            CREATE TABLE save_backup_scheduler_state (
                game_id TEXT NOT NULL,
                profile_id TEXT NOT NULL,
                enabled INTEGER NOT NULL,
                background_protection_enabled INTEGER NOT NULL,
                background_status TEXT NOT NULL,
                last_checked_at INTEGER,
                last_attempt_at INTEGER,
                last_success_at INTEGER,
                next_due_at INTEGER,
                pending_reason TEXT,
                last_error_code TEXT,
                worker_instance_id TEXT,
                worker_heartbeat_at INTEGER,
                lease_owner TEXT,
                lease_expires_at INTEGER,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (game_id, profile_id)
            );
            CREATE TABLE save_backup_background_settings (
                singleton_id INTEGER PRIMARY KEY,
                desired_enabled INTEGER NOT NULL,
                enabled_at INTEGER,
                last_worker_heartbeat_at INTEGER,
                updated_at INTEGER NOT NULL
            );
            INSERT INTO save_backups (
                backup_id, game_id, profile_id, trigger, status, archive_file_name,
                manifest_file_name, archive_size_bytes, archive_sha256, file_count,
                created_at, source_path_label, source_path_hash, notes,
                backup_directory_mode, backup_directory
            ) VALUES (
                'mhw:profile-default:1000:manual', 'mhw', 'default', 'manual',
                'completed', 'private-save.zip', 'private-save.manifest.json', 42,
                'sha256:private-archive', 2, 1000, '582010/remote',
                'sha256:private-source', 'private note', 'custom',
                'C:/Users/Player/private-backups'
            );
            INSERT INTO save_backup_scheduler_state (
                game_id, profile_id, enabled, background_protection_enabled,
                background_status, last_checked_at, last_attempt_at, last_success_at,
                next_due_at, pending_reason, last_error_code, worker_instance_id,
                worker_heartbeat_at, lease_owner, lease_expires_at, updated_at
            ) VALUES (
                'mhw', 'default', 1, 1, 'protected', 900, 910, 920, 2000,
                NULL, NULL, 'private-worker', 950, 'private-lease', 3000, 950
            );
            INSERT INTO save_backup_background_settings (
                singleton_id, desired_enabled, enabled_at,
                last_worker_heartbeat_at, updated_at
            ) VALUES (1, 1, 800, 950, 950);
            ",
        )
        .expect("write backup database fixture");
}

fn write_background_fixture(sandbox: &Path) {
    let root = sandbox.join("fixtures").join("background");
    fs::create_dir_all(&root).expect("create background fixture root");
    fs::write(
        root.join("status.json"),
        r#"{"registrationStatus":"registered","nowUnixMillis":1000}"#,
    )
    .expect("write background fixture");
}

fn write_diagnostics_fixture(sandbox: &Path) {
    let app_logs = sandbox.join("logs").join("app");
    let task_logs = sandbox.join("logs").join("tasks");
    let audit_logs = sandbox.join("logs").join("audit");
    fs::create_dir_all(&app_logs).expect("create app logs");
    fs::create_dir_all(&task_logs).expect("create task logs");
    fs::create_dir_all(&audit_logs).expect("create audit logs");
    fs::write(
        app_logs.join("app-1970-01-01.log"),
        "safe app line\nC:/Users/Player/raw_path\n",
    )
    .expect("write app log fixture");
    fs::write(
        task_logs.join("task-fixture-1.log"),
        "safe task line\ntoken=private\n",
    )
    .expect("write task log fixture");
    fs::write(
        audit_logs.join("audit-1970-01-01.log"),
        serde_json::json!({
            "timestampUnixMillis": 42,
            "category": "save_backup",
            "operation": "create_backup",
            "result": "success",
            "fields": {
                "file_count": "2"
            }
        })
        .to_string()
            + "\n",
    )
    .expect("write audit log fixture");
}

fn tree_snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn visit(root: &Path, directory: &Path, snapshot: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        let mut entries = fs::read_dir(directory)
            .expect("read snapshot directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read snapshot entries");
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("snapshot path relative to root")
                .to_path_buf();
            if path.is_dir() {
                snapshot.insert(relative, None);
                visit(root, &path, snapshot);
            } else {
                snapshot.insert(relative, Some(fs::read(path).expect("read snapshot file")));
            }
        }
    }

    let mut snapshot = BTreeMap::new();
    visit(root, root, &mut snapshot);
    snapshot
}

#[test]
fn production_runtime_status_is_read_only_json() {
    let output = hmm(&["--format", "json", "runtime", "status"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    let value: Value = serde_json::from_str(&stdout_text(&output)).expect("json output");
    assert_eq!(value["schemaVersion"], "hmm.cli/v1");
    assert_eq!(value["command"], "runtime.status");
    assert_eq!(value["ok"], true);
    assert_eq!(value["result"]["environment"], "production");
    assert_eq!(value["result"]["dataRootMode"], "system");
    assert_eq!(value["result"]["writeCommandPolicy"], "disabled");
    assert_eq!(value["result"]["productionWritesAllowed"], false);
    assert_eq!(value["result"]["businessCommandsAvailable"], true);
}

#[test]
fn sandbox_runtime_status_is_one_jsonl_record_without_path_echo() {
    let sandbox = absolute_sandbox_path();
    let output = hmm(&[
        "--format",
        "jsonl",
        "--environment",
        "sandbox",
        "--data-dir",
        sandbox.to_str().expect("sandbox path"),
        "runtime",
        "status",
    ]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert_eq!(stdout.lines().count(), 1);
    assert!(!stdout.contains(sandbox.to_str().expect("sandbox path")));
    let value: Value = serde_json::from_str(stdout.trim()).expect("jsonl record");
    assert_eq!(value["result"]["environment"], "sandbox");
    assert_eq!(value["result"]["dataRootMode"], "explicit_sandbox");
    assert_eq!(value["result"]["writeCommandPolicy"], "sandbox_only");
    assert_eq!(value["result"]["productionWritesAllowed"], false);
}

#[test]
fn sandbox_without_data_dir_returns_machine_readable_usage_error() {
    let output = hmm(&[
        "--format",
        "json",
        "--environment",
        "sandbox",
        "runtime",
        "status",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr_text(&output), "");
    let value: Value = serde_json::from_str(&stdout_text(&output)).expect("json error");
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "sandbox_data_dir_required");
    assert_eq!(value["error"]["category"], "user_action_required");
    assert_eq!(value["result"], Value::Null);
}

#[test]
fn production_rejects_data_dir_without_echoing_it() {
    let sandbox = absolute_sandbox_path();
    let output = hmm(&[
        "--format",
        "json",
        "--data-dir",
        sandbox.to_str().expect("sandbox path"),
        "runtime",
        "status",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert!(!stdout.contains(sandbox.to_str().expect("sandbox path")));
    let value: Value = serde_json::from_str(&stdout).expect("json error");
    assert_eq!(value["error"]["code"], "production_data_dir_forbidden");
}

#[test]
fn reinstall_command_is_reachable_and_requires_a_candidate_revision() {
    let output = hmm(&["install", "reinstall", "--mod", "mod-a"]);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout_text(&output), "");
    assert!(stderr_text(&output).contains("--candidate-revision"));
}

#[test]
fn machine_mode_parse_errors_use_a_stable_redacted_envelope() {
    let sandbox = absolute_sandbox_path();
    let output = hmm(&[
        "--format=json",
        "--data-dir",
        sandbox.to_str().expect("sandbox path"),
        "install",
        "apply",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert!(!stdout.contains(sandbox.to_str().expect("sandbox path")));
    assert!(!stdout.contains("install"));
    let value: Value = serde_json::from_str(&stdout).expect("json error");
    assert_eq!(value["schemaVersion"], "hmm.cli/v1");
    assert_eq!(value["command"], "cli.parse");
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "cli_usage_error");
    assert_eq!(value["error"]["category"], "user_action_required");
    assert_eq!(value["error"]["retryable"], false);
}

#[test]
fn help_is_written_to_stdout_without_initializing_runtime() {
    let output = hmm(&["--help"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert!(stdout.contains("Usage: hmm"));
    assert!(stdout.contains("runtime"));
    assert!(stdout.contains("game"));
    assert!(stdout.contains("install"));
}

#[test]
fn sandbox_game_status_returns_path_free_json() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let game_root = create_game_fixture(sandbox.path(), true);
    write_game_config(sandbox.path(), &game_root);

    let output = hmm_in_sandbox(sandbox.path(), "json", &["status", "--game", "mhw"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert!(!stdout.contains(&sandbox.path().to_string_lossy().to_string()));
    let value: Value = serde_json::from_str(&stdout).expect("status json");
    assert_eq!(value["schemaVersion"], "hmm.cli/v1");
    assert_eq!(value["command"], "game.status");
    assert_eq!(value["result"]["gameId"], "mhw");
    assert_eq!(value["result"]["status"], "configured");
    assert!(value["result"].get("rootDir").is_none());
}

#[test]
fn sandbox_game_status_human_output_uses_stable_label_value_lines() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let game_root = create_game_fixture(sandbox.path(), true);
    write_game_config(sandbox.path(), &game_root);

    let output = hmm_in_sandbox(sandbox.path(), "human", &["status", "--game", "mhw"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    assert_eq!(
        stdout_text(&output),
        "game: mhw\nstatus: configured\nerror: none\n"
    );
}

#[test]
fn sandbox_game_scan_returns_one_path_free_jsonl_record() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    write_steam_fixture(sandbox.path());

    let output = hmm_in_sandbox(sandbox.path(), "jsonl", &["scan", "--game", "mhw"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert_eq!(stdout.lines().count(), 1);
    assert!(!stdout.contains(&sandbox.path().to_string_lossy().to_string()));
    let value: Value = serde_json::from_str(stdout.trim()).expect("scan jsonl");
    assert_eq!(value["command"], "game.scan");
    assert_eq!(value["result"]["candidateCount"], 1);
    assert_eq!(value["result"]["validCandidateCount"], 1);
    assert_eq!(value["result"]["invalidCandidateCount"], 0);
}

#[test]
fn sandbox_game_validate_reports_stable_issue_codes_without_evidence_labels() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let game_root = create_game_fixture(sandbox.path(), false);
    write_game_config(sandbox.path(), &game_root);

    let output = hmm_in_sandbox(sandbox.path(), "json", &["validate", "--game", "mhw"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    let value: Value = serde_json::from_str(&stdout_text(&output)).expect("validation json");
    assert_eq!(value["command"], "game.validate");
    assert_eq!(value["result"]["state"], "validated");
    assert_eq!(value["result"]["valid"], false);
    assert_eq!(
        value["result"]["issueCodes"],
        serde_json::json!(["missing_executable"])
    );
    assert!(value["result"].get("directory").is_none());
    assert!(value["result"].get("message").is_none());
}

#[test]
fn sandbox_game_prerequisites_uses_bundled_rules_without_seeding() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let game_root = create_game_fixture(sandbox.path(), true);
    write_game_config(sandbox.path(), &game_root);
    let override_path = sandbox
        .path()
        .join("config")
        .join("prerequisite-rules")
        .join("mhw.json");

    let output = hmm_in_sandbox(sandbox.path(), "json", &["prerequisites", "--game", "mhw"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert!(!stdout.contains(&sandbox.path().to_string_lossy().to_string()));
    let value: Value = serde_json::from_str(&stdout).expect("prerequisites json");
    assert_eq!(value["command"], "game.prerequisites");
    assert_eq!(value["result"]["state"], "ready");
    assert_eq!(value["result"]["status"], "error");
    assert!(value["result"]["itemCount"].as_u64().unwrap_or_default() > 0);
    assert!(value["result"]["items"][0].get("path").is_none());
    assert!(!override_path.exists());
    assert!(!override_path.parent().expect("override parent").exists());
}

#[test]
fn sandbox_game_commands_do_not_modify_fixture_or_config_tree() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let game_root = create_game_fixture(sandbox.path(), true);
    write_game_config(sandbox.path(), &game_root);
    write_steam_fixture(sandbox.path());
    let before = tree_snapshot(sandbox.path());

    for command in [
        ["status", "--game", "mhw"],
        ["scan", "--game", "mhw"],
        ["validate", "--game", "mhw"],
        ["prerequisites", "--game", "mhw"],
    ] {
        let output = hmm_in_sandbox(sandbox.path(), "json", &command);
        assert_eq!(output.status.code(), Some(0), "{}", stderr_text(&output));
    }

    assert_eq!(tree_snapshot(sandbox.path()), before);
}

#[test]
fn sandbox_rejects_saved_game_path_outside_fixture_root_without_echoing_it() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let outside = tempfile::tempdir().expect("outside");
    fs::write(outside.path().join("MonsterHunterWorld.exe"), b"fixture").expect("outside exe");
    write_game_config(sandbox.path(), outside.path());

    let output = hmm_in_sandbox(sandbox.path(), "json", &["status", "--game", "mhw"]);

    assert_eq!(output.status.code(), Some(3));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert!(!stdout.contains(&sandbox.path().to_string_lossy().to_string()));
    assert!(!stdout.contains(&outside.path().to_string_lossy().to_string()));
    let value: Value = serde_json::from_str(&stdout).expect("rejection json");
    assert_eq!(value["command"], "game.status");
    assert_eq!(value["error"]["code"], "sandbox_game_path_rejected");
    assert_eq!(value["error"]["category"], "data_safety_risk");
}

#[test]
fn unsupported_game_returns_usage_envelope() {
    let sandbox = tempfile::tempdir().expect("sandbox");

    let output = hmm_in_sandbox(sandbox.path(), "json", &["status", "--game", "rise"]);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr_text(&output), "");
    let value: Value = serde_json::from_str(&stdout_text(&output)).expect("usage json");
    assert_eq!(value["error"]["code"], "unsupported_game");
    assert_eq!(value["error"]["category"], "user_action_required");
}

#[test]
fn sandbox_install_plan_returns_safe_relative_targets_in_json() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    write_mod_catalog_and_sandbox(sandbox.path());

    let output = hmm_install_in_sandbox(sandbox.path(), "json", &["plan", "--mod", "mod-a"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert!(!stdout.contains(&sandbox.path().to_string_lossy().to_string()));
    assert!(!stdout.contains("package-a"));
    let value: Value = serde_json::from_str(&stdout).expect("plan json");
    assert_eq!(value["schemaVersion"], "hmm.cli/v1");
    assert_eq!(value["command"], "install.plan");
    assert_eq!(value["result"]["gameId"], "mhw");
    assert_eq!(value["result"]["modId"], "mod-a");
    assert_eq!(value["result"]["actionCount"], 1);
    assert_eq!(
        value["result"]["actions"][0]["targetPath"],
        "nativePC/models/player.mod3"
    );
    assert!(value["result"]["actions"][0].get("packageFileId").is_none());
}

#[test]
fn sandbox_install_plan_blocks_missing_required_prerequisites_without_token() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let game_root = create_game_fixture(sandbox.path(), true);
    write_game_config(sandbox.path(), &game_root);
    write_mod_catalog_and_sandbox(sandbox.path());
    let before = tree_snapshot(sandbox.path());

    let output = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &["plan", "--profile", "default", "--mod", "mod-a"],
    );

    assert_eq!(output.status.code(), Some(0), "{}", stderr_text(&output));
    let value: Value =
        serde_json::from_str(&stdout_text(&output)).expect("blocked install plan json");
    assert_eq!(value["result"]["prerequisiteDecision"]["status"], "blocked");
    assert_eq!(value["result"]["prerequisiteDecision"]["rulesVersion"], 1);
    assert!(value["result"]["prerequisiteDecision"]["codes"]
        .as_array()
        .expect("prerequisite decision codes")
        .iter()
        .any(|code| code == "missing_required_file"));
    assert!(value["result"].get("planToken").is_none());
    assert!(value["result"].get("expiresAtUnixMillis").is_none());
    assert_eq!(tree_snapshot(sandbox.path()), before);
}

#[test]
fn sandbox_reinstall_preview_blocks_missing_prerequisites_without_token() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let game_root = create_game_fixture(sandbox.path(), true);
    write_game_config(sandbox.path(), &game_root);
    write_reinstall_v1_catalog_and_sandbox(sandbox.path());
    let before = tree_snapshot(sandbox.path());

    let output = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "reinstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--candidate-revision",
            "package-v1",
        ],
    );

    assert_eq!(output.status.code(), Some(0), "{}", stderr_text(&output));
    let value: Value =
        serde_json::from_str(&stdout_text(&output)).expect("blocked reinstall preview json");
    assert_eq!(value["result"]["status"], "blocked");
    assert_eq!(value["result"]["prerequisiteDecision"]["status"], "blocked");
    assert_eq!(value["result"]["prerequisiteDecision"]["rulesVersion"], 1);
    assert!(value["result"]["prerequisiteDecision"]["codes"]
        .as_array()
        .expect("prerequisite decision codes")
        .iter()
        .any(|code| code == "missing_required_file"));
    assert_eq!(
        value["result"]["blockingReasons"][0]["code"],
        "prerequisites_blocked"
    );
    assert!(value["result"].get("planToken").is_none());
    assert!(value["result"].get("expiresAtUnixMillis").is_none());
    assert_eq!(tree_snapshot(sandbox.path()), before);
}

#[test]
fn sandbox_install_status_has_stable_human_output() {
    let sandbox = tempfile::tempdir().expect("sandbox");

    let output = hmm_install_in_sandbox(
        sandbox.path(),
        "human",
        &["status", "--profile", "default", "--mod", "mod-a"],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    assert_eq!(
        stdout_text(&output),
        concat!(
            "game: none\n",
            "profile: default\n",
            "items: 1\n",
            "mod mod-a status: not_installed\n",
            "mod mod-a managed files: 0\n",
            "mod mod-a backups: 0\n"
        )
    );
}

#[test]
fn sandbox_recovery_scan_is_one_jsonl_record() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let game_root = create_game_fixture(sandbox.path(), true);
    write_game_config(sandbox.path(), &game_root);

    let output = hmm_install_in_sandbox(
        sandbox.path(),
        "jsonl",
        &["recovery", "scan", "--profile", "default"],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert_eq!(stdout.lines().count(), 1);
    assert!(!stdout.contains(&sandbox.path().to_string_lossy().to_string()));
    let value: Value = serde_json::from_str(stdout.trim()).expect("scan jsonl");
    assert_eq!(value["command"], "install.recovery.scan");
    assert_eq!(value["result"]["gameId"], "mhw");
    assert_eq!(value["result"]["profileId"], "default");
    assert_eq!(value["result"]["itemCount"], 0);
}

#[test]
fn sandbox_recovery_preview_returns_blocked_aggregate_without_refs() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let game_root = create_game_fixture(sandbox.path(), true);
    write_game_config(sandbox.path(), &game_root);

    let output = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "recovery",
            "preview",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--action",
            "rollback-install",
        ],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert!(!stdout.contains(&sandbox.path().to_string_lossy().to_string()));
    let value: Value = serde_json::from_str(&stdout).expect("preview json");
    assert_eq!(value["command"], "install.recovery.preview");
    assert_eq!(value["result"]["availability"], "blocked");
    assert_eq!(
        value["result"]["blockingReasons"][0]["code"],
        "rollback_state_missing"
    );
    assert!(value["result"].get("planToken").is_none());
    assert!(value["result"].get("expiresAtUnixMillis").is_none());
    assert!(value["result"].get("backupRef").is_none());
    assert!(value["result"].get("recoveryRef").is_none());
}

#[test]
fn sandbox_recovery_apply_rolls_back_artificial_record_across_process_restarts() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let outside = tempfile::tempdir().expect("outside");
    let sentinel = outside.path().join("sentinel.bin");
    fs::write(&sentinel, b"outside").expect("write sentinel");
    write_sandbox_marker(sandbox.path());
    let game_root = create_game_fixture(sandbox.path(), true);
    fs::create_dir_all(game_root.join("nativePC/models")).expect("recovery baseline parents");
    let game_baseline = tree_snapshot(&game_root);
    write_game_config(sandbox.path(), &game_root);
    let recovery_target = write_rollback_recovery_fixture(sandbox.path(), &game_root);
    let recovery_state = tree_snapshot(sandbox.path());

    for command in [
        vec![
            "recovery",
            "apply",
            "--profile",
            "default",
            "--mod",
            "mod-recovery",
            "--action",
            "rollback-install",
        ],
        vec![
            "recovery",
            "apply",
            "--profile",
            "default",
            "--mod",
            "mod-recovery",
            "--action",
            "rollback-install",
            "--commit",
        ],
        vec![
            "recovery",
            "apply",
            "--profile",
            "default",
            "--mod",
            "mod-recovery",
            "--action",
            "rollback-install",
            "--yes",
        ],
    ] {
        let preview = hmm_install_in_sandbox(sandbox.path(), "json", &command);
        assert_eq!(preview.status.code(), Some(0), "{}", stderr_text(&preview));
        let value: Value =
            serde_json::from_str(&stdout_text(&preview)).expect("recovery apply preview");
        assert_eq!(value["command"], "install.recovery.apply");
        assert_eq!(value["result"]["availability"], "available");
        assert_eq!(value["result"]["removeFileCount"], 1);
        assert!(value["result"]["planToken"].as_str().is_some());
        assert_eq!(tree_snapshot(sandbox.path()), recovery_state);
    }

    let missing_token = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "recovery",
            "apply",
            "--profile",
            "default",
            "--mod",
            "mod-recovery",
            "--action",
            "rollback-install",
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(missing_token.status.code(), Some(2));
    let missing_token_value: Value =
        serde_json::from_str(&stdout_text(&missing_token)).expect("missing recovery token");
    assert_eq!(missing_token_value["error"]["code"], "plan_token_required");
    assert_eq!(tree_snapshot(sandbox.path()), recovery_state);

    let preview = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "recovery",
            "preview",
            "--profile",
            "default",
            "--mod",
            "mod-recovery",
            "--action",
            "rollback-install",
        ],
    );
    let preview_value: Value =
        serde_json::from_str(&stdout_text(&preview)).expect("recovery preview token");
    let stale_token = preview_value["result"]["planToken"]
        .as_str()
        .expect("recovery token")
        .to_owned();
    let recovery_repository =
        JsonInstallRecoveryRecordRepository::new(sandbox.path().join("install/recovery"));
    let original_recovery_record = recovery_repository
        .load_record(&ProfileId::new("default"), &ModId::new("mod-recovery"))
        .expect("load recovery record")
        .expect("recovery record");
    let mut changed_recovery_record = original_recovery_record.clone();
    changed_recovery_record.entries[0].package_file_id =
        PackageFileId::new("nativePC/models/same-count-change.mod3");
    recovery_repository
        .save_record(&changed_recovery_record)
        .expect("save same-count recovery change");
    let changed_recovery_state = tree_snapshot(sandbox.path());
    let stale_apply = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "recovery",
            "apply",
            "--profile",
            "default",
            "--mod",
            "mod-recovery",
            "--action",
            "rollback-install",
            "--plan-token",
            &stale_token,
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(stale_apply.status.code(), Some(3));
    let stale_apply_value: Value =
        serde_json::from_str(&stdout_text(&stale_apply)).expect("stale recovery token rejection");
    assert_eq!(stale_apply_value["error"]["code"], "plan_token_invalid");
    assert_eq!(stale_apply_value["taskId"], Value::Null);
    assert_eq!(tree_snapshot(sandbox.path()), changed_recovery_state);
    assert_eq!(fs::read(&sentinel).expect("read sentinel"), b"outside");
    recovery_repository
        .save_record(&original_recovery_record)
        .expect("restore recovery record");

    let refreshed_preview = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "recovery",
            "preview",
            "--profile",
            "default",
            "--mod",
            "mod-recovery",
            "--action",
            "rollback-install",
        ],
    );
    let refreshed_preview_value: Value =
        serde_json::from_str(&stdout_text(&refreshed_preview)).expect("refreshed recovery preview");
    let token = refreshed_preview_value["result"]["planToken"]
        .as_str()
        .expect("refreshed recovery token")
        .to_owned();
    let apply = hmm_install_in_sandbox(
        sandbox.path(),
        "jsonl",
        &[
            "recovery",
            "apply",
            "--profile",
            "default",
            "--mod",
            "mod-recovery",
            "--action",
            "rollback-install",
            "--plan-token",
            &token,
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(apply.status.code(), Some(0), "{}", stderr_text(&apply));
    assert_eq!(stderr_text(&apply), "");
    let output = stdout_text(&apply);
    let events = output
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("recovery task event jsonl"))
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 4);
    assert_eq!(
        events
            .iter()
            .map(|event| event["sequence"].as_u64().expect("sequence"))
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert_eq!(
        events
            .iter()
            .map(|event| event["phase"].as_str().expect("phase"))
            .collect::<Vec<_>>(),
        [
            "install.recovery.queued",
            "install.recovery.planning",
            "install.recovery.processing",
            "install.recovery.completed",
        ]
    );
    for event in &events {
        assert_eq!(event["command"], "install.recovery.apply");
        assert!(!event.to_string().contains(&token));
        assert!(!event
            .to_string()
            .contains(&sandbox.path().to_string_lossy().to_string()));
        assert!(!event
            .to_string()
            .contains(&outside.path().to_string_lossy().to_string()));
    }
    assert!(!recovery_target.exists());
    assert_eq!(tree_snapshot(&game_root), game_baseline);
    let post_preview = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "recovery",
            "preview",
            "--profile",
            "default",
            "--mod",
            "mod-recovery",
            "--action",
            "rollback-install",
        ],
    );
    let post_preview_value: Value =
        serde_json::from_str(&stdout_text(&post_preview)).expect("post-recovery preview");
    assert_eq!(post_preview_value["result"]["availability"], "blocked");
    assert!(post_preview_value["result"].get("planToken").is_none());
    let audit_text = fs::read_dir(sandbox.path().join("logs/audit"))
        .expect("audit directory")
        .map(|entry| fs::read_to_string(entry.expect("audit entry").path()).expect("audit log"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(audit_text.contains("\"operation\":\"rollback_install\""));
    assert_eq!(fs::read(&sentinel).expect("sentinel remains"), b"outside");
}

#[test]
fn sandbox_install_commands_do_not_modify_fixture_or_state_tree() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let game_root = create_game_fixture(sandbox.path(), true);
    write_game_config(sandbox.path(), &game_root);
    write_mod_catalog_and_sandbox(sandbox.path());
    let before = tree_snapshot(sandbox.path());

    for command in [
        vec!["plan", "--mod", "mod-a"],
        vec!["status", "--profile", "default", "--mod", "mod-a"],
        vec!["recovery", "scan", "--profile", "default"],
        vec![
            "recovery",
            "preview",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--action",
            "rollback-install",
        ],
    ] {
        let output = hmm_install_in_sandbox(sandbox.path(), "json", &command);
        assert_eq!(output.status.code(), Some(0), "{}", stderr_text(&output));
    }

    assert_eq!(tree_snapshot(sandbox.path()), before);
    assert!(!sandbox
        .path()
        .join("mod-import")
        .join("results.json.lock")
        .exists());
    assert!(!sandbox.path().join("install").exists());
}

#[test]
fn sandbox_install_rejects_path_like_ids_without_echoing_them() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let unsafe_id = "../private-profile";

    let output = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &["status", "--profile", unsafe_id, "--mod", "mod-a"],
    );

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert!(!stdout.contains(unsafe_id));
    assert!(!stdout.contains(&sandbox.path().to_string_lossy().to_string()));
    let value: Value = serde_json::from_str(&stdout).expect("id error json");
    assert_eq!(value["command"], "install.status");
    assert_eq!(value["error"]["code"], "profile_id_invalid");
}

#[test]
fn sandbox_recovery_rejects_game_root_outside_fixtures_without_echoing_it() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let outside = tempfile::tempdir().expect("outside");
    fs::write(outside.path().join("MonsterHunterWorld.exe"), b"fixture").expect("outside exe");
    write_game_config(sandbox.path(), outside.path());

    let output = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &["recovery", "scan", "--profile", "default"],
    );

    assert_eq!(output.status.code(), Some(3));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert!(!stdout.contains(&sandbox.path().to_string_lossy().to_string()));
    assert!(!stdout.contains(&outside.path().to_string_lossy().to_string()));
    let value: Value = serde_json::from_str(&stdout).expect("containment error json");
    assert_eq!(value["command"], "install.recovery.scan");
    assert_eq!(value["error"]["code"], "sandbox_game_path_rejected");
    assert_eq!(value["error"]["category"], "data_safety_risk");
}

#[test]
fn sandbox_install_apply_requires_dual_confirmation_and_plan_token() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let game_root = create_game_fixture(sandbox.path(), true);
    write_unverified_prerequisite_fixture(&game_root);
    write_game_config(sandbox.path(), &game_root);
    write_mod_catalog_and_sandbox(sandbox.path());
    let before = tree_snapshot(sandbox.path());

    for command in [
        vec!["apply", "--profile", "default", "--mod", "mod-a"],
        vec![
            "apply",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--commit",
        ],
        vec!["apply", "--profile", "default", "--mod", "mod-a", "--yes"],
    ] {
        let output = hmm_install_in_sandbox(sandbox.path(), "json", &command);
        assert_eq!(output.status.code(), Some(0), "{}", stderr_text(&output));
        let value: Value = serde_json::from_str(&stdout_text(&output)).expect("dry-run apply json");
        assert_eq!(value["command"], "install.apply");
        assert_eq!(value["result"]["profileId"], "default");
        assert_eq!(value["result"]["prerequisiteDecision"]["status"], "warning");
        assert!(value["result"]["prerequisiteDecision"]["codes"]
            .as_array()
            .expect("prerequisite warning codes")
            .iter()
            .any(|code| code == "signature_unverified"));
        assert!(value["result"]["planToken"].as_str().is_some());
        assert!(value["result"]["expiresAtUnixMillis"].as_u64().is_some());
        assert_eq!(tree_snapshot(sandbox.path()), before);
    }

    let missing_token = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "apply",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(missing_token.status.code(), Some(2));
    let value: Value =
        serde_json::from_str(&stdout_text(&missing_token)).expect("missing token json");
    assert_eq!(value["error"]["code"], "plan_token_required");
    assert_eq!(tree_snapshot(sandbox.path()), before);
}

#[test]
fn sandbox_install_apply_binary_writes_only_fixture_tree_and_emits_jsonl() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let outside = tempfile::tempdir().expect("outside");
    let sentinel = outside.path().join("sentinel.bin");
    fs::write(&sentinel, b"outside").expect("write sentinel");
    write_sandbox_marker(sandbox.path());
    let game_root = create_game_fixture(sandbox.path(), true);
    write_unverified_prerequisite_fixture(&game_root);
    fs::create_dir_all(game_root.join("nativePC/models")).expect("baseline target parents");
    write_game_config(sandbox.path(), &game_root);
    write_mod_catalog_and_sandbox(sandbox.path());
    let game_baseline = tree_snapshot(&game_root);

    let plan = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &["plan", "--profile", "default", "--mod", "mod-a"],
    );
    assert_eq!(plan.status.code(), Some(0), "{}", stderr_text(&plan));
    let plan_value: Value = serde_json::from_str(&stdout_text(&plan)).expect("install plan json");
    let token = plan_value["result"]["planToken"]
        .as_str()
        .expect("sandbox plan token")
        .to_owned();

    let apply = hmm_install_in_sandbox(
        sandbox.path(),
        "jsonl",
        &[
            "apply",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--plan-token",
            &token,
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(apply.status.code(), Some(0), "{}", stderr_text(&apply));
    assert_eq!(stderr_text(&apply), "");
    let output = stdout_text(&apply);
    let events = output
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("task event jsonl"))
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 4);
    assert_eq!(
        events
            .iter()
            .map(|event| event["sequence"].as_u64().expect("sequence"))
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert_eq!(
        events
            .iter()
            .map(|event| event["phase"].as_str().expect("phase"))
            .collect::<Vec<_>>(),
        [
            "install.queued",
            "install.plan.building",
            "install.commit.processing",
            "install.completed",
        ]
    );
    for event in &events {
        assert_eq!(event["command"], "install.apply");
        assert!(!event.to_string().contains(&token));
        assert!(!event
            .to_string()
            .contains(&sandbox.path().to_string_lossy().to_string()));
        assert!(!event
            .to_string()
            .contains(&outside.path().to_string_lossy().to_string()));
    }
    assert_eq!(
        fs::read(game_root.join("nativePC/models/player.mod3")).expect("installed fixture"),
        b"fixture"
    );
    assert!(sandbox
        .path()
        .join("install/manifests/default.json")
        .exists());

    let installed_tree = tree_snapshot(sandbox.path());
    for command in [
        vec!["uninstall", "--profile", "default", "--mod", "mod-a"],
        vec![
            "uninstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--commit",
        ],
        vec![
            "uninstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--yes",
        ],
    ] {
        let preview = hmm_install_in_sandbox(sandbox.path(), "json", &command);
        assert_eq!(preview.status.code(), Some(0), "{}", stderr_text(&preview));
        let preview_value: Value =
            serde_json::from_str(&stdout_text(&preview)).expect("uninstall preview json");
        assert_eq!(preview_value["command"], "install.uninstall");
        assert_eq!(preview_value["result"]["status"], "installed");
        assert_eq!(preview_value["result"]["available"], true);
        assert_eq!(preview_value["result"]["managedFileCount"], 1);
        assert!(preview_value["result"]["planToken"].as_str().is_some());
        assert_eq!(tree_snapshot(sandbox.path()), installed_tree);
    }

    let missing_uninstall_token = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "uninstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(missing_uninstall_token.status.code(), Some(2));
    let missing_uninstall_token_value: Value =
        serde_json::from_str(&stdout_text(&missing_uninstall_token))
            .expect("missing uninstall token json");
    assert_eq!(
        missing_uninstall_token_value["error"]["code"],
        "plan_token_required"
    );
    assert_eq!(tree_snapshot(sandbox.path()), installed_tree);

    let uninstall_preview = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &["uninstall", "--profile", "default", "--mod", "mod-a"],
    );
    let uninstall_preview_value: Value =
        serde_json::from_str(&stdout_text(&uninstall_preview)).expect("uninstall preview json");
    let stale_uninstall_token = uninstall_preview_value["result"]["planToken"]
        .as_str()
        .expect("uninstall token")
        .to_owned();
    let manifest_repository =
        JsonInstallManifestRepository::new(sandbox.path().join("install/manifests"));
    let original_manifest = manifest_repository
        .load_manifest(&ProfileId::new("default"))
        .expect("load install manifest")
        .expect("installed manifest");
    let mut changed_manifest = original_manifest.clone();
    changed_manifest.entries[0].package_file_id =
        PackageFileId::new("nativePC/models/same-count-change.mod3");
    manifest_repository
        .save_manifest(&changed_manifest)
        .expect("save same-count manifest change");
    let changed_manifest_state = tree_snapshot(sandbox.path());
    let stale_uninstall = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "uninstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--plan-token",
            &stale_uninstall_token,
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(stale_uninstall.status.code(), Some(3));
    let stale_uninstall_value: Value = serde_json::from_str(&stdout_text(&stale_uninstall))
        .expect("stale uninstall token rejection");
    assert_eq!(stale_uninstall_value["error"]["code"], "plan_token_invalid");
    assert_eq!(stale_uninstall_value["taskId"], Value::Null);
    assert_eq!(tree_snapshot(sandbox.path()), changed_manifest_state);
    assert_eq!(fs::read(&sentinel).expect("read sentinel"), b"outside");
    manifest_repository
        .save_manifest(&original_manifest)
        .expect("restore install manifest");

    let refreshed_uninstall_preview = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &["uninstall", "--profile", "default", "--mod", "mod-a"],
    );
    let refreshed_uninstall_preview_value: Value =
        serde_json::from_str(&stdout_text(&refreshed_uninstall_preview))
            .expect("refreshed uninstall preview");
    let uninstall_token = refreshed_uninstall_preview_value["result"]["planToken"]
        .as_str()
        .expect("refreshed uninstall token")
        .to_owned();
    let uninstall = hmm_install_in_sandbox(
        sandbox.path(),
        "jsonl",
        &[
            "uninstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--plan-token",
            &uninstall_token,
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(
        uninstall.status.code(),
        Some(0),
        "{}",
        stderr_text(&uninstall)
    );
    assert_eq!(stderr_text(&uninstall), "");
    let uninstall_output = stdout_text(&uninstall);
    let uninstall_events = uninstall_output
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("uninstall task event jsonl"))
        .collect::<Vec<_>>();
    assert_eq!(uninstall_events.len(), 3);
    assert_eq!(
        uninstall_events
            .iter()
            .map(|event| event["sequence"].as_u64().expect("sequence"))
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(
        uninstall_events
            .iter()
            .map(|event| event["phase"].as_str().expect("phase"))
            .collect::<Vec<_>>(),
        [
            "install.uninstall.queued",
            "install.uninstall.processing",
            "install.uninstall.completed",
        ]
    );
    for event in &uninstall_events {
        assert_eq!(event["command"], "install.uninstall");
        assert!(!event.to_string().contains(&uninstall_token));
        assert!(!event
            .to_string()
            .contains(&sandbox.path().to_string_lossy().to_string()));
        assert!(!event
            .to_string()
            .contains(&outside.path().to_string_lossy().to_string()));
    }

    assert_eq!(tree_snapshot(&game_root), game_baseline);
    let status = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "status",
            "--game",
            "mhw",
            "--profile",
            "default",
            "--mod",
            "mod-a",
        ],
    );
    assert_eq!(status.status.code(), Some(0), "{}", stderr_text(&status));
    let status_value: Value =
        serde_json::from_str(&stdout_text(&status)).expect("post-uninstall status json");
    assert_eq!(
        status_value["result"]["items"][0]["status"],
        "not_installed"
    );
    let audit_text = fs::read_dir(sandbox.path().join("logs/audit"))
        .expect("audit directory")
        .map(|entry| fs::read_to_string(entry.expect("audit entry").path()).expect("audit log"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(audit_text.contains("\"operation\":\"commit_imported_mod\""));
    assert!(audit_text.contains("\"operation\":\"uninstall_mod\""));
    assert!(
        sandbox
            .path()
            .join("logs/tasks")
            .read_dir()
            .expect("task logs")
            .count()
            >= 2
    );
    assert_eq!(fs::read(&sentinel).expect("sentinel remains"), b"outside");
}

#[test]
fn sandbox_install_batch_plan_returns_preview_token_without_sensitive_fields() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let game_root = create_game_fixture(sandbox.path(), true);
    write_unverified_prerequisite_fixture(&game_root);
    write_game_config(sandbox.path(), &game_root);
    write_mod_catalog_and_sandbox(sandbox.path());
    let before = tree_snapshot(sandbox.path());

    let output = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "batch",
            "plan",
            "--profile",
            "default",
            "--item",
            "mod-a:package-a",
        ],
    );

    assert_eq!(output.status.code(), Some(0), "{}", stderr_text(&output));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert!(!stdout.contains(&sandbox.path().to_string_lossy().to_string()));
    let value: Value = serde_json::from_str(&stdout).expect("batch plan json");
    assert_eq!(value["schemaVersion"], "hmm.cli/v1");
    assert_eq!(value["command"], "install.batch.plan");
    assert_eq!(value["result"]["plan"]["operation"], "install");
    assert_eq!(value["result"]["plan"]["gameId"], "mhw");
    assert_eq!(value["result"]["plan"]["profileId"], "default");
    assert_eq!(
        value["result"]["plan"]["items"][0]["revisionId"],
        "package-a"
    );
    assert_eq!(
        value["result"]["plan"]["items"][0]["modId"],
        "mod-a"
    );
    assert!(value["result"]["plan"].get("environmentDigest").is_none());
    assert!(value["result"]["plan"].get("batchDigest").is_none());
    assert!(value["result"]["plan"]["items"][0]
        .get("factDigest")
        .is_none());
    assert!(value["result"]["plan"]["items"][0]
        .get("singlePlanDigest")
        .is_none());
    assert!(value["result"]["plan"]["items"][0]
        .get("targetClaims")
        .is_none());
    assert!(value["result"]["previewToken"].as_str().is_some());
    assert!(value["result"]["expiresAtUnixMillis"].as_u64().is_some());
    assert_eq!(tree_snapshot(sandbox.path()), before);
}

#[test]
fn sandbox_install_batch_apply_requires_commit_yes_and_preview_token() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let game_root = create_game_fixture(sandbox.path(), true);
    write_unverified_prerequisite_fixture(&game_root);
    write_game_config(sandbox.path(), &game_root);
    write_mod_catalog_and_sandbox(sandbox.path());
    let before = tree_snapshot(sandbox.path());

    for command in [
        vec![
            "batch",
            "apply",
            "--profile",
            "default",
            "--item",
            "mod-a:package-a",
        ],
        vec![
            "batch",
            "apply",
            "--profile",
            "default",
            "--item",
            "mod-a:package-a",
            "--commit",
        ],
        vec![
            "batch",
            "apply",
            "--profile",
            "default",
            "--item",
            "mod-a:package-a",
            "--yes",
        ],
    ] {
        let output = hmm_install_in_sandbox(sandbox.path(), "json", &command);
        assert_eq!(output.status.code(), Some(2), "{}", stderr_text(&output));
        assert_eq!(stderr_text(&output), "");
        let value: Value =
            serde_json::from_str(&stdout_text(&output)).expect("batch confirmation json");
        assert_eq!(value["command"], "install.batch.apply");
        assert_eq!(value["error"]["code"], "batch_commit_required");
        assert_eq!(tree_snapshot(sandbox.path()), before);
    }

    let plan = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "batch",
            "plan",
            "--profile",
            "default",
            "--item",
            "mod-a:package-a",
        ],
    );
    let plan_value: Value =
        serde_json::from_str(&stdout_text(&plan)).expect("batch plan json");
    let missing_token = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "batch",
            "apply",
            "--profile",
            "default",
            "--item",
            "mod-a:package-a",
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(missing_token.status.code(), Some(2));
    let missing_token_value: Value =
        serde_json::from_str(&stdout_text(&missing_token)).expect("missing batch token json");
    assert_eq!(
        missing_token_value["error"]["code"],
        "batch_preview_token_required"
    );
    assert!(plan_value["result"]["previewToken"].as_str().is_some());
    assert_eq!(tree_snapshot(sandbox.path()), before);
}

#[test]
fn production_install_batch_apply_is_rejected_before_runtime_write_admission() {
    let output = hmm(&[
        "--format",
        "json",
        "--environment",
        "production",
        "install",
        "batch",
        "apply",
        "--item",
        "mod-a:package-a",
        "--preview-token",
        "opaque-preview-token",
        "--commit",
        "--yes",
    ]);

    assert_eq!(output.status.code(), Some(3));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert!(!stdout.contains("opaque-preview-token"));
    let value: Value = serde_json::from_str(&stdout).expect("production batch rejection json");
    assert_eq!(value["command"], "install.batch.apply");
    assert_eq!(
        value["error"]["code"],
        "sandbox_batch_production_forbidden"
    );
    assert_eq!(value["error"]["category"], "data_safety_risk");
}

#[test]
fn sandbox_install_batch_apply_result_and_completed_retry_are_stable_across_processes() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let outside = tempfile::tempdir().expect("outside");
    let sentinel = outside.path().join("sentinel.bin");
    fs::write(&sentinel, b"outside").expect("write sentinel");
    write_sandbox_marker(sandbox.path());
    let game_root = create_game_fixture(sandbox.path(), true);
    write_unverified_prerequisite_fixture(&game_root);
    fs::create_dir_all(game_root.join("nativePC/models")).expect("create target parent");
    write_game_config(sandbox.path(), &game_root);
    write_mod_catalog_and_sandbox(sandbox.path());
    let game_before = tree_snapshot(&game_root);

    let plan = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "batch",
            "plan",
            "--profile",
            "default",
            "--item",
            "mod-a:package-a",
        ],
    );
    assert_eq!(plan.status.code(), Some(0), "{}", stderr_text(&plan));
    let plan_value: Value =
        serde_json::from_str(&stdout_text(&plan)).expect("batch plan json");
    let preview_token = plan_value["result"]["previewToken"]
        .as_str()
        .expect("batch preview token")
        .to_owned();

    let apply = hmm_install_in_sandbox(
        sandbox.path(),
        "jsonl",
        &[
            "batch",
            "apply",
            "--profile",
            "default",
            "--item",
            "mod-a:package-a",
            "--preview-token",
            &preview_token,
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(apply.status.code(), Some(0), "{}", stderr_text(&apply));
    assert_eq!(stderr_text(&apply), "");
    let apply_stdout = stdout_text(&apply);
    assert_eq!(apply_stdout.lines().count(), 1);
    assert!(!apply_stdout.contains(&sandbox.path().to_string_lossy().to_string()));
    assert!(!apply_stdout.contains("package-a"));
    let apply_value: Value =
        serde_json::from_str(apply_stdout.trim()).expect("batch apply jsonl");
    assert_eq!(apply_value["command"], "install.batch.apply");
    assert_eq!(apply_value["result"]["status"], "completed");
    assert_eq!(apply_value["result"]["summary"]["succeeded_count"], 1);
    let batch_id = apply_value["result"]["batchId"]
        .as_str()
        .expect("batch id")
        .to_owned();
    assert!(apply_value["result"]["taskId"].as_str().is_some());
    assert_ne!(tree_snapshot(&game_root), game_before);
    assert_eq!(fs::read(&sentinel).expect("sentinel remains"), b"outside");

    let result = hmm_install_in_sandbox(
        sandbox.path(),
        "jsonl",
        &[
            "batch",
            "result",
            "--batch-id",
            &batch_id,
            "--attempt",
            "0",
        ],
    );
    assert_eq!(result.status.code(), Some(0), "{}", stderr_text(&result));
    assert_eq!(stderr_text(&result), "");
    let result_stdout = stdout_text(&result);
    assert_eq!(result_stdout.lines().count(), 1);
    assert!(!result_stdout.contains(&sandbox.path().to_string_lossy().to_string()));
    assert!(!result_stdout.contains("package-a"));
    let result_value: Value =
        serde_json::from_str(result_stdout.trim()).expect("batch result jsonl");
    assert_eq!(result_value["command"], "install.batch.result");
    assert_eq!(result_value["result"]["batchId"], batch_id);
    assert_eq!(result_value["result"]["status"], "completed");
    assert_eq!(result_value["result"]["items"][0]["status"], "succeeded");
    let game_after_result = tree_snapshot(&game_root);

    let retry = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "batch",
            "retry",
            "--batch-id",
            &batch_id,
            "--attempt",
            "0",
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(retry.status.code(), Some(3));
    assert_eq!(stderr_text(&retry), "");
    let retry_value: Value =
        serde_json::from_str(&stdout_text(&retry)).expect("completed retry rejection json");
    assert_eq!(retry_value["command"], "install.batch.retry");
    assert_eq!(retry_value["error"]["code"], "batch_retry_unavailable");
    assert_eq!(tree_snapshot(&game_root), game_after_result);
    assert_eq!(fs::read(&sentinel).expect("sentinel remains"), b"outside");
}

#[test]
fn sandbox_install_batch_apply_rejects_stale_preview_before_sealing_or_writing() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    write_sandbox_marker(sandbox.path());
    let game_root = create_game_fixture(sandbox.path(), true);
    write_unverified_prerequisite_fixture(&game_root);
    write_game_config(sandbox.path(), &game_root);
    write_mod_catalog_and_sandbox(sandbox.path());

    let plan = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "batch",
            "plan",
            "--profile",
            "default",
            "--item",
            "mod-a:package-a",
        ],
    );
    let plan_value: Value =
        serde_json::from_str(&stdout_text(&plan)).expect("batch plan json");
    let preview_token = plan_value["result"]["previewToken"]
        .as_str()
        .expect("batch preview token")
        .to_owned();

    fs::remove_file(game_root.join("dinput8.dll")).expect("mutate prerequisites after preview");
    let before_apply = tree_snapshot(sandbox.path());

    let apply = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "batch",
            "apply",
            "--profile",
            "default",
            "--item",
            "mod-a:package-a",
            "--preview-token",
            &preview_token,
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(apply.status.code(), Some(3));
    assert_eq!(stderr_text(&apply), "");
    let value: Value = serde_json::from_str(&stdout_text(&apply)).expect("stale batch json");
    assert_eq!(value["command"], "install.batch.apply");
    assert_eq!(value["error"]["code"], "batch_plan_stale");
    assert_eq!(tree_snapshot(sandbox.path()), before_apply);
}

#[test]
fn sandbox_reinstall_binary_replaces_revision_and_restores_exact_baseline() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let outside = tempfile::tempdir().expect("outside");
    let sentinel = outside.path().join("sentinel.bin");
    fs::write(&sentinel, b"outside").expect("write sentinel");
    write_sandbox_marker(sandbox.path());
    let game_root = create_game_fixture(sandbox.path(), true);
    write_unverified_prerequisite_fixture(&game_root);
    let models_root = game_root.join("nativePC/models");
    fs::create_dir_all(&models_root).expect("create reinstall fixture parent");
    fs::write(models_root.join("replaced.mod3"), b"game-replaced")
        .expect("write replaced baseline");
    fs::write(models_root.join("stale.mod3"), b"game-stale").expect("write stale baseline");
    write_game_config(sandbox.path(), &game_root);
    write_reinstall_v1_catalog_and_sandbox(sandbox.path());
    let game_baseline = tree_snapshot(&game_root);

    let install_plan = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &["plan", "--profile", "default", "--mod", "mod-a"],
    );
    assert_eq!(
        install_plan.status.code(),
        Some(0),
        "{}",
        stderr_text(&install_plan)
    );
    let install_plan_value: Value =
        serde_json::from_str(&stdout_text(&install_plan)).expect("install plan json");
    let install_token = install_plan_value["result"]["planToken"]
        .as_str()
        .expect("install token")
        .to_owned();
    let install = hmm_install_in_sandbox(
        sandbox.path(),
        "jsonl",
        &[
            "apply",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--plan-token",
            &install_token,
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(install.status.code(), Some(0), "{}", stderr_text(&install));
    assert_eq!(
        fs::read(models_root.join("replaced.mod3")).expect("installed v1 replacement"),
        b"revision-v1"
    );

    let same_revision = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "reinstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--candidate-revision",
            "package-v1",
        ],
    );
    assert_eq!(
        same_revision.status.code(),
        Some(0),
        "{}",
        stderr_text(&same_revision)
    );
    let same_revision_value: Value =
        serde_json::from_str(&stdout_text(&same_revision)).expect("same revision preview");
    assert_eq!(same_revision_value["result"]["status"], "blocked");
    assert!(same_revision_value["result"].get("planToken").is_none());
    assert_eq!(
        same_revision_value["result"]["blockingReasons"][0]["code"],
        "candidate_already_installed"
    );

    append_reinstall_v2_revision(sandbox.path());
    let before_reinstall = tree_snapshot(sandbox.path());
    for command in [
        vec![
            "reinstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--candidate-revision",
            "revision-v2",
        ],
        vec![
            "reinstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--candidate-revision",
            "revision-v2",
            "--commit",
        ],
        vec![
            "reinstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--candidate-revision",
            "revision-v2",
            "--yes",
        ],
    ] {
        let preview = hmm_install_in_sandbox(sandbox.path(), "json", &command);
        assert_eq!(preview.status.code(), Some(0), "{}", stderr_text(&preview));
        let preview_value: Value =
            serde_json::from_str(&stdout_text(&preview)).expect("reinstall preview json");
        assert_eq!(preview_value["command"], "install.reinstall");
        assert_eq!(preview_value["result"]["status"], "ready");
        assert_eq!(
            preview_value["result"]["prerequisiteDecision"]["status"],
            "warning"
        );
        assert_eq!(
            preview_value["result"]["prerequisiteDecision"]["rulesVersion"],
            1
        );
        assert!(preview_value["result"]["prerequisiteDecision"]["codes"]
            .as_array()
            .expect("prerequisite warning codes")
            .iter()
            .any(|code| code == "signature_unverified"));
        assert_eq!(preview_value["result"]["installedRevisionId"], "package-v1");
        assert_eq!(
            preview_value["result"]["candidateRevisionId"],
            "revision-v2"
        );
        assert_eq!(preview_value["result"]["retainedCount"], 1);
        assert_eq!(preview_value["result"]["replacedCount"], 1);
        assert_eq!(preview_value["result"]["addedCount"], 1);
        assert_eq!(preview_value["result"]["staleCount"], 1);
        assert!(preview_value["result"]["blockingReasons"]
            .as_array()
            .expect("blocking reasons")
            .is_empty());
        assert!(preview_value["result"]["planToken"].as_str().is_some());
        assert!(preview_value["result"]["expiresAtUnixMillis"]
            .as_u64()
            .is_some());
        assert_eq!(tree_snapshot(sandbox.path()), before_reinstall);
    }

    let missing_token = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "reinstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--candidate-revision",
            "revision-v2",
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(missing_token.status.code(), Some(2));
    let missing_token_value: Value =
        serde_json::from_str(&stdout_text(&missing_token)).expect("missing token json");
    assert_eq!(missing_token_value["command"], "install.reinstall");
    assert_eq!(missing_token_value["error"]["code"], "plan_token_required");
    assert_eq!(tree_snapshot(sandbox.path()), before_reinstall);

    let preview = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "reinstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--candidate-revision",
            "revision-v2",
        ],
    );
    let preview_value: Value =
        serde_json::from_str(&stdout_text(&preview)).expect("reinstall token preview");
    let reinstall_token = preview_value["result"]["planToken"]
        .as_str()
        .expect("reinstall token")
        .to_owned();
    let reinstall = hmm_install_in_sandbox(
        sandbox.path(),
        "jsonl",
        &[
            "reinstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--candidate-revision",
            "revision-v2",
            "--plan-token",
            &reinstall_token,
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(
        reinstall.status.code(),
        Some(0),
        "{}",
        stderr_text(&reinstall)
    );
    assert_eq!(stderr_text(&reinstall), "");
    let reinstall_events = stdout_text(&reinstall)
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("reinstall task event jsonl"))
        .collect::<Vec<_>>();
    assert_eq!(reinstall_events.len(), 5);
    assert_eq!(
        reinstall_events
            .iter()
            .map(|event| event["sequence"].as_u64().expect("sequence"))
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4]
    );
    assert_eq!(
        reinstall_events
            .iter()
            .map(|event| event["phase"].as_str().expect("phase"))
            .collect::<Vec<_>>(),
        [
            "install.reinstall.queued",
            "install.reinstall.plan.building",
            "install.reinstall.preflight.processing",
            "install.reinstall.commit.processing",
            "install.reinstall.completed",
        ]
    );
    for event in &reinstall_events {
        assert_eq!(event["command"], "install.reinstall");
        assert!(!event.to_string().contains(&reinstall_token));
        assert!(!event
            .to_string()
            .contains(&sandbox.path().to_string_lossy().to_string()));
        assert!(!event
            .to_string()
            .contains(&outside.path().to_string_lossy().to_string()));
    }

    assert_eq!(
        fs::read(models_root.join("retained.mod3")).expect("retained file"),
        b"same"
    );
    assert_eq!(
        fs::read(models_root.join("replaced.mod3")).expect("replaced file"),
        b"revision-v2"
    );
    assert_eq!(
        fs::read(models_root.join("added.mod3")).expect("added file"),
        b"revision-v2-added"
    );
    assert_eq!(
        fs::read(models_root.join("stale.mod3")).expect("restored stale baseline"),
        b"game-stale"
    );

    let manifest =
        JsonInstallManifestRepository::new(sandbox.path().join("install").join("manifests"))
            .load_manifest(&ProfileId::new("default"))
            .expect("load manifest")
            .expect("manifest exists");
    let mod_entries = manifest
        .entries
        .iter()
        .filter(|entry| entry.mod_id == ModId::new("mod-a"))
        .collect::<Vec<_>>();
    assert_eq!(mod_entries.len(), 3);
    assert!(mod_entries
        .iter()
        .all(|entry| { entry.revision_id.as_ref() == Some(&ModRevisionId::new("revision-v2")) }));
    assert!(!mod_entries
        .iter()
        .any(|entry| entry.target_path.as_str().ends_with("stale.mod3")));

    let audit_text = fs::read_dir(sandbox.path().join("logs/audit"))
        .expect("audit directory")
        .map(|entry| fs::read_to_string(entry.expect("audit entry").path()).expect("audit log"))
        .collect::<String>();
    assert!(audit_text.contains("\"operation\":\"reinstall_mod\""));
    assert!(!audit_text.contains(&reinstall_token));
    assert!(
        sandbox
            .path()
            .join("logs/tasks")
            .read_dir()
            .expect("task logs")
            .count()
            >= 2
    );

    let uninstall_preview = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &["uninstall", "--profile", "default", "--mod", "mod-a"],
    );
    let uninstall_preview_value: Value =
        serde_json::from_str(&stdout_text(&uninstall_preview)).expect("uninstall preview");
    let uninstall_token = uninstall_preview_value["result"]["planToken"]
        .as_str()
        .expect("uninstall token")
        .to_owned();
    let uninstall = hmm_install_in_sandbox(
        sandbox.path(),
        "jsonl",
        &[
            "uninstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--plan-token",
            &uninstall_token,
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(
        uninstall.status.code(),
        Some(0),
        "{}",
        stderr_text(&uninstall)
    );
    assert_eq!(tree_snapshot(&game_root), game_baseline);
    assert_eq!(fs::read(&sentinel).expect("sentinel remains"), b"outside");
}

#[test]
fn sandbox_reinstall_rejects_stale_token_before_task_or_game_write() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let outside = tempfile::tempdir().expect("outside");
    let sentinel = outside.path().join("sentinel.bin");
    fs::write(&sentinel, b"outside").expect("write sentinel");
    write_sandbox_marker(sandbox.path());
    let game_root = create_game_fixture(sandbox.path(), true);
    write_unverified_prerequisite_fixture(&game_root);
    fs::create_dir_all(game_root.join("nativePC/models")).expect("create game parent");
    write_game_config(sandbox.path(), &game_root);
    write_reinstall_v1_catalog_and_sandbox(sandbox.path());

    let install_plan = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &["plan", "--profile", "default", "--mod", "mod-a"],
    );
    let install_plan_value: Value =
        serde_json::from_str(&stdout_text(&install_plan)).expect("install plan json");
    let install_token = install_plan_value["result"]["planToken"]
        .as_str()
        .expect("install token")
        .to_owned();
    let install = hmm_install_in_sandbox(
        sandbox.path(),
        "jsonl",
        &[
            "apply",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--plan-token",
            &install_token,
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(install.status.code(), Some(0), "{}", stderr_text(&install));

    append_reinstall_v2_revision(sandbox.path());
    let preview = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "reinstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--candidate-revision",
            "revision-v2",
        ],
    );
    let preview_value: Value =
        serde_json::from_str(&stdout_text(&preview)).expect("reinstall preview");
    let token = preview_value["result"]["planToken"]
        .as_str()
        .expect("reinstall token")
        .to_owned();

    fs::write(
        sandbox
            .path()
            .join("mod-import/sandboxes/package-v2/nativePC/models/replaced.mod3"),
        b"revision-v2-mutated",
    )
    .expect("mutate candidate after preview");
    let game_before = tree_snapshot(&game_root);
    let sandbox_before = tree_snapshot(sandbox.path());
    let apply = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "reinstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--candidate-revision",
            "revision-v2",
            "--plan-token",
            &token,
            "--commit",
            "--yes",
        ],
    );

    assert_eq!(apply.status.code(), Some(3));
    assert_eq!(stderr_text(&apply), "");
    let apply_value: Value =
        serde_json::from_str(&stdout_text(&apply)).expect("stale token rejection");
    assert_eq!(apply_value["command"], "install.reinstall");
    assert_eq!(apply_value["error"]["code"], "plan_token_invalid");
    assert!(apply_value["taskId"].is_null());
    assert_eq!(tree_snapshot(&game_root), game_before);
    assert_eq!(tree_snapshot(sandbox.path()), sandbox_before);
    assert_eq!(fs::read(&sentinel).expect("sentinel remains"), b"outside");
}

#[cfg(windows)]
#[test]
#[allow(clippy::permissions_set_readonly_false)]
fn sandbox_reinstall_manifest_save_failure_rolls_back_v1_in_real_binary() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let outside = tempfile::tempdir().expect("outside");
    let sentinel = outside.path().join("sentinel.bin");
    fs::write(&sentinel, b"outside").expect("write sentinel");
    write_sandbox_marker(sandbox.path());
    let game_root = create_game_fixture(sandbox.path(), true);
    write_unverified_prerequisite_fixture(&game_root);
    fs::create_dir_all(game_root.join("nativePC/models")).expect("create game parent");
    write_game_config(sandbox.path(), &game_root);
    write_reinstall_v1_catalog_and_sandbox(sandbox.path());
    let game_baseline = tree_snapshot(&game_root);

    let install_plan = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &["plan", "--profile", "default", "--mod", "mod-a"],
    );
    let install_plan_value: Value =
        serde_json::from_str(&stdout_text(&install_plan)).expect("install plan json");
    let install_token = install_plan_value["result"]["planToken"]
        .as_str()
        .expect("install token")
        .to_owned();
    let install = hmm_install_in_sandbox(
        sandbox.path(),
        "jsonl",
        &[
            "apply",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--plan-token",
            &install_token,
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(install.status.code(), Some(0), "{}", stderr_text(&install));
    let installed_v1 = tree_snapshot(&game_root);

    append_reinstall_v2_revision(sandbox.path());
    let preview = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &[
            "reinstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--candidate-revision",
            "revision-v2",
        ],
    );
    let preview_value: Value =
        serde_json::from_str(&stdout_text(&preview)).expect("reinstall preview");
    let token = preview_value["result"]["planToken"]
        .as_str()
        .expect("reinstall token")
        .to_owned();

    let manifest_path = sandbox.path().join("install/manifests/default.json");
    let mut permissions = fs::metadata(&manifest_path)
        .expect("manifest metadata")
        .permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&manifest_path, permissions).expect("make manifest read-only");
    let reinstall = hmm_install_in_sandbox(
        sandbox.path(),
        "jsonl",
        &[
            "reinstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--candidate-revision",
            "revision-v2",
            "--plan-token",
            &token,
            "--commit",
            "--yes",
        ],
    );
    let mut permissions = fs::metadata(&manifest_path)
        .expect("manifest remains")
        .permissions();
    permissions.set_readonly(false);
    fs::set_permissions(&manifest_path, permissions).expect("restore manifest permissions");

    assert_eq!(
        reinstall.status.code(),
        Some(4),
        "{}",
        stderr_text(&reinstall)
    );
    assert_eq!(stderr_text(&reinstall), "");
    let events = stdout_text(&reinstall)
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("reinstall failure event"))
        .collect::<Vec<_>>();
    assert_eq!(
        events
            .iter()
            .map(|event| event["phase"].as_str().expect("phase"))
            .collect::<Vec<_>>(),
        [
            "install.reinstall.queued",
            "install.reinstall.plan.building",
            "install.reinstall.preflight.processing",
            "install.reinstall.commit.processing",
            "install.reinstall.rollback.processing",
            "install.reinstall.failed",
        ]
    );
    assert_eq!(
        events.last().expect("failed terminal")["error"]["code"],
        "install_reinstall_failed:manifest"
    );
    assert_eq!(tree_snapshot(&game_root), installed_v1);

    let manifest =
        JsonInstallManifestRepository::new(sandbox.path().join("install").join("manifests"))
            .load_manifest(&ProfileId::new("default"))
            .expect("load rolled-back manifest")
            .expect("v1 manifest remains");
    assert!(manifest
        .entries
        .iter()
        .filter(|entry| entry.mod_id == ModId::new("mod-a"))
        .all(|entry| entry.revision_id.is_none()));

    let recovery = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &["recovery", "scan", "--profile", "default", "--mod", "mod-a"],
    );
    assert_eq!(
        recovery.status.code(),
        Some(0),
        "{}",
        stderr_text(&recovery)
    );
    let recovery_value: Value =
        serde_json::from_str(&stdout_text(&recovery)).expect("recovery scan");
    assert_eq!(recovery_value["result"]["items"][0]["status"], "installed");

    let uninstall_preview = hmm_install_in_sandbox(
        sandbox.path(),
        "json",
        &["uninstall", "--profile", "default", "--mod", "mod-a"],
    );
    let uninstall_preview_value: Value =
        serde_json::from_str(&stdout_text(&uninstall_preview)).expect("uninstall preview");
    let uninstall_token = uninstall_preview_value["result"]["planToken"]
        .as_str()
        .expect("uninstall token")
        .to_owned();
    let uninstall = hmm_install_in_sandbox(
        sandbox.path(),
        "jsonl",
        &[
            "uninstall",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--plan-token",
            &uninstall_token,
            "--commit",
            "--yes",
        ],
    );
    assert_eq!(
        uninstall.status.code(),
        Some(0),
        "{}",
        stderr_text(&uninstall)
    );
    assert_eq!(tree_snapshot(&game_root), game_baseline);
    assert_eq!(fs::read(&sentinel).expect("sentinel remains"), b"outside");
}

#[test]
fn production_install_apply_is_rejected_before_runtime_write_admission() {
    let output = hmm(&[
        "--format",
        "json",
        "--environment",
        "production",
        "install",
        "apply",
        "--mod",
        "mod-a",
        "--plan-token",
        "hmm-lifecycle-plan-v1:0000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "--commit",
        "--yes",
    ]);

    assert_eq!(output.status.code(), Some(3));
    assert_eq!(stderr_text(&output), "");
    let value: Value =
        serde_json::from_str(&stdout_text(&output)).expect("production rejection json");
    assert_eq!(value["command"], "install.apply");
    assert_eq!(value["error"]["code"], "production_write_command_forbidden");
    assert_eq!(value["error"]["category"], "data_safety_risk");

    let uninstall = hmm(&[
        "--format",
        "json",
        "--environment",
        "production",
        "install",
        "uninstall",
        "--mod",
        "mod-a",
        "--plan-token",
        "hmm-lifecycle-plan-v1:0000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "--commit",
        "--yes",
    ]);
    assert_eq!(uninstall.status.code(), Some(3));
    assert_eq!(stderr_text(&uninstall), "");
    let uninstall_value: Value =
        serde_json::from_str(&stdout_text(&uninstall)).expect("production uninstall rejection");
    assert_eq!(uninstall_value["command"], "install.uninstall");
    assert_eq!(
        uninstall_value["error"]["code"],
        "production_write_command_forbidden"
    );
    assert_eq!(uninstall_value["error"]["category"], "data_safety_risk");

    let reinstall = hmm(&[
        "--format",
        "json",
        "--environment",
        "production",
        "install",
        "reinstall",
        "--mod",
        "mod-a",
        "--candidate-revision",
        "revision-v2",
        "--plan-token",
        "hmm-lifecycle-plan-v1:0000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "--commit",
        "--yes",
    ]);
    assert_eq!(reinstall.status.code(), Some(3));
    assert_eq!(stderr_text(&reinstall), "");
    let reinstall_value: Value =
        serde_json::from_str(&stdout_text(&reinstall)).expect("production reinstall rejection");
    assert_eq!(reinstall_value["command"], "install.reinstall");
    assert_eq!(
        reinstall_value["error"]["code"],
        "production_write_command_forbidden"
    );
    assert_eq!(reinstall_value["error"]["category"], "data_safety_risk");

    let recovery = hmm(&[
        "--format",
        "json",
        "--environment",
        "production",
        "install",
        "recovery",
        "apply",
        "--profile",
        "default",
        "--mod",
        "mod-a",
        "--action",
        "rollback-install",
        "--plan-token",
        "hmm-lifecycle-plan-v1:0000000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "--commit",
        "--yes",
    ]);
    assert_eq!(recovery.status.code(), Some(3));
    assert_eq!(stderr_text(&recovery), "");
    let recovery_value: Value =
        serde_json::from_str(&stdout_text(&recovery)).expect("production recovery rejection");
    assert_eq!(recovery_value["command"], "install.recovery.apply");
    assert_eq!(
        recovery_value["error"]["code"],
        "production_write_command_forbidden"
    );
    assert_eq!(recovery_value["error"]["category"], "data_safety_risk");
}

#[test]
fn sandbox_backup_list_returns_sanitized_json_without_archive_details() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    write_backup_database(sandbox.path());

    let output = hmm_backup_in_sandbox(
        sandbox.path(),
        "json",
        &["list", "--profile", "default", "--limit", "10"],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    for forbidden in [
        sandbox.path().to_string_lossy().as_ref(),
        "private-save.zip",
        "private-save.manifest.json",
        "sha256:private",
        "582010/remote",
        "private note",
        "C:/Users/Player",
    ] {
        assert!(!stdout.contains(forbidden));
    }
    let value: Value = serde_json::from_str(&stdout).expect("backup list json");
    assert_eq!(value["schemaVersion"], "hmm.cli/v1");
    assert_eq!(value["command"], "backup.list");
    assert_eq!(value["result"]["gameId"], "mhw");
    assert_eq!(value["result"]["profileId"], "default");
    assert_eq!(value["result"]["itemCount"], 1);
    assert_eq!(
        value["result"]["items"][0]["backupId"],
        "mhw:profile-default:1000:manual"
    );
    assert_eq!(value["result"]["items"][0]["sizeBytes"], 42);
    assert!(value["result"]["items"][0].get("fileName").is_none());
    assert!(value["result"]["items"][0].get("sourcePathLabel").is_none());
    assert!(value["result"]["items"][0].get("notes").is_none());
}

#[test]
fn sandbox_backup_background_status_has_stable_human_output() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    write_backup_database(sandbox.path());
    write_background_fixture(sandbox.path());

    let output = hmm_backup_in_sandbox(
        sandbox.path(),
        "human",
        &["background", "status", "--profile", "default"],
    );

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    assert_eq!(
        stdout_text(&output),
        concat!(
            "game: mhw\n",
            "profile: default\n",
            "status: protected\n",
            "background protection enabled: true\n",
            "last checked at: 900\n",
            "last attempt at: 910\n",
            "last success at: 920\n",
            "next due at: 2000\n",
            "pending reason: none\n",
            "error: none\n"
        )
    );
}

#[test]
fn sandbox_diagnostics_snapshot_is_one_sanitized_jsonl_record() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    write_diagnostics_fixture(sandbox.path());

    let output = hmm_diagnostics_in_sandbox(sandbox.path(), "jsonl", &["snapshot"]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert_eq!(stdout.lines().count(), 1);
    for forbidden in [
        sandbox.path().to_string_lossy().as_ref(),
        "safe app line",
        "safe task line",
        "C:/Users/Player",
        "raw_path",
        "token=private",
        "app-1970-01-01.log",
        "task-fixture-1.log",
        "create_backup",
        "file_count",
    ] {
        assert!(!stdout.contains(forbidden));
    }
    let value: Value = serde_json::from_str(stdout.trim()).expect("diagnostics jsonl");
    assert_eq!(value["command"], "diagnostics.snapshot");
    assert_eq!(value["result"]["platformStatus"], "ok");
    assert_eq!(value["result"]["appLogStatus"], "ok");
    assert_eq!(value["result"]["taskLogStatus"], "ok");
    assert_eq!(value["result"]["auditLogStatus"], "ok");
    assert_eq!(value["result"]["appLogLineCount"], 1);
    assert_eq!(value["result"]["taskLogLineCount"], 1);
    assert_eq!(value["result"]["auditEventCount"], 1);
    assert_eq!(
        value["result"]["platform"]["gameAdapterIds"],
        serde_json::json!(["mhw"])
    );
    assert!(value["result"].get("appLogLines").is_none());
    assert!(value["result"].get("auditEvents").is_none());
    assert!(value["result"].get("exportPath").is_none());
}

#[test]
fn sandbox_backup_and_diagnostics_commands_do_not_modify_state_tree() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    write_backup_database(sandbox.path());
    write_background_fixture(sandbox.path());
    write_diagnostics_fixture(sandbox.path());
    let before = tree_snapshot(sandbox.path());

    for (group, command) in [
        ("backup", vec!["list", "--profile", "default"]),
        (
            "backup",
            vec!["background", "status", "--profile", "default"],
        ),
        ("diagnostics", vec!["snapshot"]),
    ] {
        let output = hmm_group_in_sandbox(sandbox.path(), "json", group, &command);
        assert_eq!(output.status.code(), Some(0), "{}", stderr_text(&output));
    }

    assert_eq!(tree_snapshot(sandbox.path()), before);
    assert!(!sandbox.path().join("hmm.db-wal").exists());
    assert!(!sandbox.path().join("hmm.db-shm").exists());
}

#[test]
fn missing_backup_database_fails_without_creating_state() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    let before = tree_snapshot(sandbox.path());

    let output = hmm_backup_in_sandbox(sandbox.path(), "json", &["list", "--profile", "default"]);

    assert_eq!(output.status.code(), Some(6));
    assert_eq!(stderr_text(&output), "");
    let value: Value = serde_json::from_str(&stdout_text(&output)).expect("database error json");
    assert_eq!(value["command"], "backup.list");
    assert_eq!(value["error"]["code"], "backup_database_unavailable");
    assert_eq!(tree_snapshot(sandbox.path()), before);
    assert!(!sandbox.path().join("hmm.db").exists());
}

#[test]
fn backup_list_rejects_wal_sidecars_without_echoing_or_modifying_state() {
    let sandbox = tempfile::tempdir().expect("sandbox");
    write_backup_database(sandbox.path());
    fs::write(sandbox.path().join("hmm.db-wal"), b"private WAL fixture")
        .expect("write WAL fixture");
    fs::write(sandbox.path().join("hmm.db-shm"), b"private SHM fixture")
        .expect("write SHM fixture");
    let before = tree_snapshot(sandbox.path());

    let output = hmm_backup_in_sandbox(sandbox.path(), "json", &["list", "--profile", "default"]);

    assert_eq!(output.status.code(), Some(6));
    assert_eq!(stderr_text(&output), "");
    let stdout = stdout_text(&output);
    assert!(!stdout.contains(sandbox.path().to_string_lossy().as_ref()));
    assert!(!stdout.contains("private WAL fixture"));
    assert!(!stdout.contains("private SHM fixture"));
    let value: Value = serde_json::from_str(&stdout).expect("WAL sidecar error json");
    assert_eq!(value["command"], "backup.list");
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "backup_database_unavailable");
    assert_eq!(value["error"]["category"], "recoverable");
    assert_eq!(value["error"]["retryable"], true);
    assert!(value["error"].get("message").is_none());
    assert_eq!(tree_snapshot(sandbox.path()), before);
}

#[test]
fn backup_and_diagnostics_write_commands_remain_unreachable_at_parser_boundary() {
    for command in [
        vec!["backup", "create"],
        vec!["backup", "restore"],
        vec!["backup", "background", "enable"],
        vec!["backup", "background", "disable"],
        vec!["diagnostics", "export"],
    ] {
        let output = hmm(&command);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(stdout_text(&output), "");
        assert!(stderr_text(&output).contains("unrecognized subcommand"));
    }
}
