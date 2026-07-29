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
fn unimplemented_business_commands_are_rejected_by_the_parser() {
    let output = hmm(&["install", "apply"]);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout_text(&output), "");
    assert!(stderr_text(&output).contains("unrecognized subcommand 'apply'"));
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
    assert!(value["result"].get("backupRef").is_none());
    assert!(value["result"].get("recoveryRef").is_none());
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
fn install_write_commands_remain_unreachable_at_parser_boundary() {
    for command in [
        vec!["install", "apply"],
        vec!["install", "uninstall"],
        vec!["install", "reinstall"],
        vec!["install", "recovery", "apply"],
    ] {
        let output = hmm(&command);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(stdout_text(&output), "");
        assert!(stderr_text(&output).contains("unrecognized subcommand"));
    }
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
