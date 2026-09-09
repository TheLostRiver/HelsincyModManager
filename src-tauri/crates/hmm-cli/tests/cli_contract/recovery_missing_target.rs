use super::*;
use hmm_core::{installed_file_summary, InstallManifest, InstallManifestEntry};
use hmm_infra::FileSystemInstallBackupStore;
use hmm_ports::InstallBackupStore;

#[test]
fn missing_target_recovery_binary_requires_confirmation_and_keeps_preview_read_only() {
    for with_backup in [false, true] {
        let sandbox = tempfile::tempdir().unwrap();
        write_sandbox_marker(sandbox.path());
        let game_root = create_game_fixture(sandbox.path(), true);
        fs::create_dir_all(game_root.join("nativePC/models")).unwrap();
        fs::write(game_root.join("nativePC/foreign.bin"), b"foreign sentinel").unwrap();
        write_game_config(sandbox.path(), &game_root);
        write_mod_catalog_and_sandbox(sandbox.path());
        let target = InstallTargetPath::parse("nativePC/models/player.mod3", ["nativePC"]).unwrap();
        let backup_ref = with_backup.then(|| {
            FileSystemInstallBackupStore::new(sandbox.path().join("install/backups"))
                .store_backup(&target, b"original fixture")
                .unwrap()
        });
        JsonInstallManifestRepository::new(sandbox.path().join("install/manifests"))
            .save_manifest(&InstallManifest::completed(
                ProfileId::new("default"),
                vec![InstallManifestEntry {
                    target_path: target.clone(),
                    mod_id: ModId::new("mod-a"),
                    revision_id: None,
                    package_file_id: PackageFileId::new(target.as_str()),
                    layer: FileLayer::new("base", 0),
                    backup_ref,
                    installed_file: Some(installed_file_summary(b"fixture")),
                    adopted: false,
                }],
            ))
            .unwrap();
        let before = tree_snapshot(sandbox.path());
        let preview_args = [
            "recovery",
            "preview",
            "--profile",
            "default",
            "--mod",
            "mod-a",
            "--action",
            "uninstall-missing-targets",
        ];
        let preview = hmm_install_in_sandbox(sandbox.path(), "json", &preview_args);
        assert_eq!(preview.status.code(), Some(0), "{}", stderr_text(&preview));
        let stdout = stdout_text(&preview);
        let value: Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(value["result"]["action"], "uninstall_missing_targets");
        assert_eq!(value["result"]["availability"], "available");
        assert_eq!(value["result"]["missingFileCount"], 1);
        assert_eq!(value["result"]["removeFileCount"], 0);
        assert_eq!(
            value["result"]["restoreFileCount"],
            usize::from(with_backup)
        );
        let token = value["result"]["planToken"].as_str().unwrap();
        assert!(token.starts_with("hmm-lifecycle-plan-v1:"));
        assert!(
            !stdout.contains("missing-uninstall-v1:"),
            "internal token is not a CLI input"
        );
        assert!(!stdout.contains(&sandbox.path().to_string_lossy().to_string()));
        let human = hmm_install_in_sandbox(sandbox.path(), "human", &preview_args);
        assert_eq!(human.status.code(), Some(0));
        assert!(stdout_text(&human).contains("missing files: 1"));
        assert_eq!(tree_snapshot(sandbox.path()), before);

        for confirmation in [vec![], vec!["--commit"], vec!["--yes"]] {
            let mut args = vec![
                "recovery",
                "apply",
                "--profile",
                "default",
                "--mod",
                "mod-a",
                "--action",
                "uninstall-missing-targets",
                "--plan-token",
                token,
            ];
            args.extend(confirmation);
            let output = hmm_install_in_sandbox(sandbox.path(), "json", &args);
            assert_eq!(output.status.code(), Some(0), "{}", stderr_text(&output));
            assert_eq!(
                tree_snapshot(sandbox.path()),
                before,
                "partial confirmation must remain read-only"
            );
        }

        let result = hmm_install_in_sandbox(
            sandbox.path(),
            "jsonl",
            &[
                "recovery",
                "apply",
                "--profile",
                "default",
                "--mod",
                "mod-a",
                "--action",
                "uninstall-missing-targets",
                "--plan-token",
                token,
                "--commit",
                "--yes",
            ],
        );
        assert_eq!(result.status.code(), Some(0), "{}", stderr_text(&result));
        let lines: Vec<Value> = stdout_text(&result)
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(lines.last().unwrap()["status"], "completed");
        let target_file = game_root.join(target.as_str());
        if with_backup {
            assert_eq!(fs::read(target_file).unwrap(), b"original fixture");
        } else {
            assert!(!target_file.exists());
        }
        assert_eq!(
            fs::read(game_root.join("nativePC/foreign.bin")).unwrap(),
            b"foreign sentinel"
        );
        let manifest = JsonInstallManifestRepository::new(sandbox.path().join("install/manifests"))
            .load_manifest(&ProfileId::new("default"))
            .unwrap()
            .unwrap();
        assert!(manifest.entries.is_empty());

        let logs: Vec<String> = fs::read_dir(sandbox.path().join("logs/audit"))
            .unwrap()
            .map(|entry| fs::read_to_string(entry.unwrap().path()).unwrap())
            .collect();
        let audit: Vec<Value> = logs
            .iter()
            .flat_map(|log| log.lines())
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let recovery = audit
            .iter()
            .find(|event| event["operation"] == "uninstall_missing_targets")
            .unwrap();
        assert_eq!(recovery["result"], "success");
        assert_eq!(recovery["fields"]["remove_file_count"], "0");
        assert_eq!(
            recovery["fields"]["restore_file_count"],
            if with_backup { "1" } else { "0" }
        );
        assert!(!logs.join("").contains(token));
        assert!(!logs.join("").contains("missing-uninstall-v1:"));
    }
}
