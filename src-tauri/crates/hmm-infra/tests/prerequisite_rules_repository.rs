use hmm_core::GameId;
use hmm_ports::{GamePrerequisiteRuleRepository, GamePrerequisiteRuleRepositoryError};

const DEFAULT_RULES: &str = r#"{
  "version": 1,
  "gameId": "mhw",
  "prerequisites": []
}"#;

#[test]
fn prerequisite_repository_seeds_default_file_when_missing() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp
        .path()
        .join("config")
        .join("prerequisite-rules")
        .join("mhw.json");
    let repo = hmm_infra::JsonGamePrerequisiteRuleRepository::new(path.clone());

    let rules = repo
        .load_rules(&GameId::mhw(), DEFAULT_RULES)
        .expect("seed rules");

    assert_eq!(rules.game_id, GameId::mhw());
    assert!(path.exists());
    assert_eq!(
        std::fs::read_to_string(path).expect("read seeded file"),
        DEFAULT_RULES
    );
}

#[test]
fn prerequisite_repository_returns_storage_corrupted_for_invalid_json() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("mhw.json");
    std::fs::write(&path, "{ broken json").expect("write broken file");
    let repo = hmm_infra::JsonGamePrerequisiteRuleRepository::new(path);

    let error = repo
        .load_rules(&GameId::mhw(), DEFAULT_RULES)
        .expect_err("invalid json should fail");

    assert_eq!(error, GamePrerequisiteRuleRepositoryError::StorageCorrupted);
}

#[test]
fn prerequisite_repository_returns_storage_corrupted_for_game_id_mismatch() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("mhw.json");
    std::fs::write(&path, r#"{"version":1,"gameId":"rise","prerequisites":[]}"#)
        .expect("write mismatched file");
    let repo = hmm_infra::JsonGamePrerequisiteRuleRepository::new(path);

    let error = repo
        .load_rules(&GameId::mhw(), DEFAULT_RULES)
        .expect_err("mismatched game id should fail");

    assert_eq!(error, GamePrerequisiteRuleRepositoryError::StorageCorrupted);
}

#[test]
fn read_only_prerequisite_repository_uses_bundled_rules_without_seeding() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp
        .path()
        .join("config")
        .join("prerequisite-rules")
        .join("mhw.json");
    let repo = hmm_infra::ReadOnlyJsonGamePrerequisiteRuleRepository::new(path.clone());

    let rules = repo
        .load_rules(&GameId::mhw(), DEFAULT_RULES)
        .expect("load bundled rules");

    assert_eq!(rules.game_id, GameId::mhw());
    assert!(!path.exists());
    assert!(!path.parent().expect("rules parent").exists());
}

#[test]
fn read_only_prerequisite_repository_reads_existing_override_without_modifying_it() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("mhw.json");
    std::fs::write(&path, DEFAULT_RULES).expect("write override");
    let before = std::fs::metadata(&path)
        .expect("override metadata")
        .modified()
        .expect("override modified time");
    let repo = hmm_infra::ReadOnlyJsonGamePrerequisiteRuleRepository::new(path.clone());

    let rules = repo
        .load_rules(&GameId::mhw(), "{ invalid bundled fallback")
        .expect("load override");

    assert_eq!(rules.game_id, GameId::mhw());
    assert_eq!(
        std::fs::read_to_string(&path).expect("read unchanged override"),
        DEFAULT_RULES
    );
    assert_eq!(
        std::fs::metadata(path)
            .expect("override metadata")
            .modified()
            .expect("override modified time"),
        before
    );
}

#[test]
fn prerequisite_repository_rejects_paths_that_escape_the_game_root() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("mhw.json");
    std::fs::write(
        &path,
        r#"{
          "version": 1,
          "gameId": "mhw",
          "prerequisites": [{
            "id": "unsafe",
            "displayName": "Unsafe",
            "requiredFiles": ["../outside.txt"],
            "signatureFiles": [],
            "jsonChecks": []
          }]
        }"#,
    )
    .expect("write unsafe rules");
    let repo = hmm_infra::ReadOnlyJsonGamePrerequisiteRuleRepository::new(path);

    let error = repo
        .load_rules(&GameId::mhw(), DEFAULT_RULES)
        .expect_err("unsafe paths must fail closed");

    assert_eq!(error, GamePrerequisiteRuleRepositoryError::StorageCorrupted);
}

#[test]
fn prerequisite_repository_rejects_path_like_item_codes() {
    let temp = tempfile::tempdir().expect("temp dir");
    let path = temp.path().join("mhw.json");
    std::fs::write(
        &path,
        r#"{
          "version": 1,
          "gameId": "mhw",
          "prerequisites": [{
            "id": "C:/Users/private",
            "displayName": "Unsafe",
            "requiredFiles": [],
            "signatureFiles": [],
            "jsonChecks": []
          }]
        }"#,
    )
    .expect("write unsafe rules");
    let repo = hmm_infra::ReadOnlyJsonGamePrerequisiteRuleRepository::new(path);

    let error = repo
        .load_rules(&GameId::mhw(), DEFAULT_RULES)
        .expect_err("unsafe item code must fail closed");

    assert_eq!(error, GamePrerequisiteRuleRepositoryError::StorageCorrupted);
}
