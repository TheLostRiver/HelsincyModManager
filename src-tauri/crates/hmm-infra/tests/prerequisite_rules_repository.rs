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
    std::fs::write(
        &path,
        r#"{"version":1,"gameId":"rise","prerequisites":[]}"#,
    )
    .expect("write mismatched file");
    let repo = hmm_infra::JsonGamePrerequisiteRuleRepository::new(path);

    let error = repo
        .load_rules(&GameId::mhw(), DEFAULT_RULES)
        .expect_err("mismatched game id should fail");

    assert_eq!(error, GamePrerequisiteRuleRepositoryError::StorageCorrupted);
}
