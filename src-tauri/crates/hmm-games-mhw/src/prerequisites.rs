use hmm_core::GameId;
use hmm_ports::{
    summarize_prerequisite_items, GameDirectoryProbe, GamePrerequisiteIssue,
    GamePrerequisiteIssueCode, GamePrerequisiteItem, GamePrerequisiteItemStatus,
    GamePrerequisiteJsonCheckRule, GamePrerequisiteReport, GamePrerequisiteRule,
    GamePrerequisiteRuleSet,
};

pub fn inspect_mhw_prerequisites(
    probe: &dyn GameDirectoryProbe,
    rules: GamePrerequisiteRuleSet,
) -> GamePrerequisiteReport {
    let items = rules
        .prerequisites
        .into_iter()
        .map(|rule| inspect_rule(probe, rule))
        .collect::<Vec<_>>();

    GamePrerequisiteReport::ready(GameId::mhw(), summarize_prerequisite_items(&items), items)
}

fn inspect_rule(
    probe: &dyn GameDirectoryProbe,
    rule: GamePrerequisiteRule,
) -> GamePrerequisiteItem {
    let mut item = GamePrerequisiteItem::new(
        rule.id,
        rule.display_name,
        GamePrerequisiteItemStatus::InstalledVerified,
    );

    for path in &rule.required_files {
        if !probe.is_file(path) {
            item.status = GamePrerequisiteItemStatus::Missing;
            item.issues.push(GamePrerequisiteIssue::new(
                GamePrerequisiteIssueCode::MissingRequiredFile,
                path.clone(),
            ));
        }
    }

    if item.status == GamePrerequisiteItemStatus::Missing {
        return item;
    }

    for json_check in &rule.json_checks {
        if let Some(misconfigured) = inspect_json_check(probe, json_check) {
            item.status = GamePrerequisiteItemStatus::Misconfigured;
            item.issues.push(misconfigured);
            return item;
        }
    }

    for signature_rule in &rule.signature_files {
        let digest = match probe.sha256_hex(&signature_rule.path) {
            Ok(digest) => digest,
            Err(_) => {
                mark_unverified(&mut item, &signature_rule.path);
                continue;
            }
        };

        if !signature_rule
            .sha256
            .iter()
            .any(|known| known.eq_ignore_ascii_case(&digest))
        {
            mark_unverified(&mut item, &signature_rule.path);
        }
    }

    item
}

fn inspect_json_check(
    probe: &dyn GameDirectoryProbe,
    json_check: &GamePrerequisiteJsonCheckRule,
) -> Option<GamePrerequisiteIssue> {
    let content = match probe.read_text_file(&json_check.path) {
        Ok(content) => content,
        Err(_) => {
            return Some(GamePrerequisiteIssue::new(
                GamePrerequisiteIssueCode::ConfigReadFailed,
                json_check.path.clone(),
            ));
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(parsed) => parsed,
        Err(_) => {
            return Some(GamePrerequisiteIssue::new(
                GamePrerequisiteIssueCode::ConfigInvalidJson,
                json_check.path.clone(),
            ));
        }
    };

    for (field, expected) in &json_check.required_boolean_fields {
        if parsed.get(field).and_then(|value| value.as_bool()) != Some(*expected) {
            return Some(GamePrerequisiteIssue::new(
                GamePrerequisiteIssueCode::ConfigFieldMismatch,
                json_check.path.clone(),
            ));
        }
    }

    None
}

fn mark_unverified(item: &mut GamePrerequisiteItem, path: &str) {
    item.status = GamePrerequisiteItemStatus::InstalledUnverified;
    item.issues.push(GamePrerequisiteIssue::new(
        GamePrerequisiteIssueCode::SignatureUnverified,
        path.to_owned(),
    ));
}

#[cfg(test)]
mod tests {
    use super::inspect_mhw_prerequisites;
    use hmm_core::GameId;
    use hmm_ports::{
        GameDirectoryProbe, GamePrerequisiteIssueCode, GamePrerequisiteItemStatus,
        GamePrerequisiteJsonCheckRule, GamePrerequisiteReportState, GamePrerequisiteRule,
        GamePrerequisiteRuleSet, GamePrerequisiteSignatureRule, GamePrerequisiteSummaryStatus,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    #[test]
    fn prerequisite_report_marks_verified_when_files_hashes_and_config_match() {
        let probe = FakeProbe::default()
            .with_hashed_file(
                "dinput8.dll",
                "6e38baff0bddc5014046e3ba5a733814f95f65d5ca67e2fb15d18c5106d4e059",
            )
            .with_hashed_file(
                "loader.dll",
                "17ec93d9d57809e4968666961caf996f7d819c05b280fbb6d444b95920a801ee",
            )
            .with_hashed_file(
                "nativePC/plugins/MonsterLoader.dll",
                "f307fd30c30d708980990062c0344c0034fb4363bb6fb85d8217e0134cea7d9b",
            )
            .with_hashed_file(
                "nativePC/plugins/QuestLoader.dll",
                "97380a19c12822c318ebc7ef09df601823cbf33ec674e1aee9f8a690d5422c08",
            )
            .with_hashed_file(
                "nativePC/plugins/!CRCBypass.dll",
                "6f5ec7d28b9ee4cfbb341b778b710f3646caeba1a213ff0db85281e1a972d058",
            )
            .with_text(
                "loader-config.json",
                r#"{"enablePluginLoader":true,"logLevel":"ERROR"}"#,
            );

        let report = inspect_mhw_prerequisites(&probe, fake_rules());

        assert_eq!(report.state, GamePrerequisiteReportState::Ready);
        assert_eq!(
            report.summary_status,
            Some(GamePrerequisiteSummaryStatus::Verified)
        );
        assert_eq!(
            report.items[0].status,
            GamePrerequisiteItemStatus::InstalledVerified
        );
        assert_eq!(
            report.items[1].status,
            GamePrerequisiteItemStatus::InstalledVerified
        );
    }

    #[test]
    fn prerequisite_report_marks_missing_when_required_file_is_absent() {
        let probe = FakeProbe::default();

        let report = inspect_mhw_prerequisites(&probe, fake_rules());

        assert_eq!(report.items[0].status, GamePrerequisiteItemStatus::Missing);
        assert!(report.items[0]
            .issues
            .iter()
            .any(|issue| issue.code == GamePrerequisiteIssueCode::MissingRequiredFile));
        assert_eq!(
            report.summary_status,
            Some(GamePrerequisiteSummaryStatus::Error)
        );
    }

    #[test]
    fn prerequisite_report_marks_misconfigured_when_loader_config_is_invalid_json() {
        let probe = FakeProbe::default()
            .with_hashed_file(
                "dinput8.dll",
                "6e38baff0bddc5014046e3ba5a733814f95f65d5ca67e2fb15d18c5106d4e059",
            )
            .with_hashed_file(
                "loader.dll",
                "17ec93d9d57809e4968666961caf996f7d819c05b280fbb6d444b95920a801ee",
            )
            .with_hashed_file(
                "nativePC/plugins/MonsterLoader.dll",
                "f307fd30c30d708980990062c0344c0034fb4363bb6fb85d8217e0134cea7d9b",
            )
            .with_hashed_file(
                "nativePC/plugins/QuestLoader.dll",
                "97380a19c12822c318ebc7ef09df601823cbf33ec674e1aee9f8a690d5422c08",
            )
            .with_text("loader-config.json", "{not-json");

        let report = inspect_mhw_prerequisites(&probe, fake_rules());

        assert_eq!(
            report.items[0].status,
            GamePrerequisiteItemStatus::Misconfigured
        );
        assert!(report.items[0]
            .issues
            .iter()
            .any(|issue| issue.code == GamePrerequisiteIssueCode::ConfigInvalidJson));
    }

    #[test]
    fn prerequisite_report_marks_misconfigured_when_loader_config_field_mismatches() {
        let probe = FakeProbe::default()
            .with_hashed_file(
                "dinput8.dll",
                "6e38baff0bddc5014046e3ba5a733814f95f65d5ca67e2fb15d18c5106d4e059",
            )
            .with_hashed_file(
                "loader.dll",
                "17ec93d9d57809e4968666961caf996f7d819c05b280fbb6d444b95920a801ee",
            )
            .with_hashed_file(
                "nativePC/plugins/MonsterLoader.dll",
                "f307fd30c30d708980990062c0344c0034fb4363bb6fb85d8217e0134cea7d9b",
            )
            .with_hashed_file(
                "nativePC/plugins/QuestLoader.dll",
                "97380a19c12822c318ebc7ef09df601823cbf33ec674e1aee9f8a690d5422c08",
            )
            .with_text("loader-config.json", r#"{"enablePluginLoader":false}"#);

        let report = inspect_mhw_prerequisites(&probe, fake_rules());

        assert_eq!(
            report.items[0].status,
            GamePrerequisiteItemStatus::Misconfigured
        );
        assert!(report.items[0]
            .issues
            .iter()
            .any(|issue| issue.code == GamePrerequisiteIssueCode::ConfigFieldMismatch));
    }

    #[test]
    fn prerequisite_report_marks_unverified_when_hash_does_not_match_known_signatures() {
        let probe = FakeProbe::default()
            .with_hashed_file("dinput8.dll", "wrong-dinput8")
            .with_hashed_file(
                "loader.dll",
                "17ec93d9d57809e4968666961caf996f7d819c05b280fbb6d444b95920a801ee",
            )
            .with_hashed_file(
                "nativePC/plugins/MonsterLoader.dll",
                "f307fd30c30d708980990062c0344c0034fb4363bb6fb85d8217e0134cea7d9b",
            )
            .with_hashed_file(
                "nativePC/plugins/QuestLoader.dll",
                "97380a19c12822c318ebc7ef09df601823cbf33ec674e1aee9f8a690d5422c08",
            )
            .with_hashed_file(
                "nativePC/plugins/!CRCBypass.dll",
                "6f5ec7d28b9ee4cfbb341b778b710f3646caeba1a213ff0db85281e1a972d058",
            )
            .with_text("loader-config.json", r#"{"enablePluginLoader":true}"#);

        let report = inspect_mhw_prerequisites(&probe, fake_rules());

        assert_eq!(
            report.items[0].status,
            GamePrerequisiteItemStatus::InstalledUnverified
        );
        assert!(report.items[0]
            .issues
            .iter()
            .any(|issue| issue.code == GamePrerequisiteIssueCode::SignatureUnverified));
        assert_eq!(
            report.summary_status,
            Some(GamePrerequisiteSummaryStatus::Warning)
        );
    }

    fn fake_rules() -> GamePrerequisiteRuleSet {
        GamePrerequisiteRuleSet {
            version: 1,
            game_id: GameId::mhw(),
            prerequisites: vec![
                GamePrerequisiteRule {
                    id: "stracker_loader".to_owned(),
                    display_name: "Stracker's Loader".to_owned(),
                    required_files: vec![
                        "dinput8.dll".to_owned(),
                        "loader.dll".to_owned(),
                        "loader-config.json".to_owned(),
                        "nativePC/plugins/MonsterLoader.dll".to_owned(),
                        "nativePC/plugins/QuestLoader.dll".to_owned(),
                    ],
                    signature_files: vec![
                        GamePrerequisiteSignatureRule {
                            path: "dinput8.dll".to_owned(),
                            sha256: vec![
                                "6E38BAFF0BDDC5014046E3BA5A733814F95F65D5CA67E2FB15D18C5106D4E059"
                                    .to_owned(),
                            ],
                        },
                        GamePrerequisiteSignatureRule {
                            path: "loader.dll".to_owned(),
                            sha256: vec![
                                "17EC93D9D57809E4968666961CAF996F7D819C05B280FBB6D444B95920A801EE"
                                    .to_owned(),
                            ],
                        },
                        GamePrerequisiteSignatureRule {
                            path: "nativePC/plugins/MonsterLoader.dll".to_owned(),
                            sha256: vec![
                                "F307FD30C30D708980990062C0344C0034FB4363BB6FB85D8217E0134CEA7D9B"
                                    .to_owned(),
                            ],
                        },
                        GamePrerequisiteSignatureRule {
                            path: "nativePC/plugins/QuestLoader.dll".to_owned(),
                            sha256: vec![
                                "97380A19C12822C318EBC7EF09DF601823CBF33EC674E1AEE9F8A690D5422C08"
                                    .to_owned(),
                            ],
                        },
                    ],
                    json_checks: vec![GamePrerequisiteJsonCheckRule {
                        path: "loader-config.json".to_owned(),
                        required_boolean_fields: BTreeMap::from([(
                            "enablePluginLoader".to_owned(),
                            true,
                        )]),
                    }],
                },
                GamePrerequisiteRule {
                    id: "crc_bypass".to_owned(),
                    display_name: "CRCBypass".to_owned(),
                    required_files: vec!["nativePC/plugins/!CRCBypass.dll".to_owned()],
                    signature_files: vec![GamePrerequisiteSignatureRule {
                        path: "nativePC/plugins/!CRCBypass.dll".to_owned(),
                        sha256: vec![
                            "6F5EC7D28B9EE4CFBB341B778B710F3646CAEBA1A213FF0DB85281E1A972D058"
                                .to_owned(),
                        ],
                    }],
                    json_checks: Vec::new(),
                },
            ],
        }
    }

    #[derive(Default)]
    struct FakeProbe {
        files: BTreeSet<String>,
        texts: BTreeMap<String, String>,
        hashes: BTreeMap<String, String>,
        root_dir: PathBuf,
    }

    impl FakeProbe {
        fn with_hashed_file(mut self, path: &str, sha256: &str) -> Self {
            self.files.insert(path.to_owned());
            self.hashes.insert(path.to_owned(), sha256.to_owned());
            self
        }

        fn with_text(mut self, path: &str, content: &str) -> Self {
            self.files.insert(path.to_owned());
            self.texts.insert(path.to_owned(), content.to_owned());
            self
        }
    }

    impl GameDirectoryProbe for FakeProbe {
        fn root_dir(&self) -> &Path {
            &self.root_dir
        }

        fn root_exists(&self) -> bool {
            true
        }

        fn exists(&self, relative_path: &str) -> bool {
            self.files.contains(relative_path)
        }

        fn is_file(&self, relative_path: &str) -> bool {
            self.files.contains(relative_path)
        }

        fn is_dir(&self, _relative_path: &str) -> bool {
            false
        }

        fn read_text_file(&self, relative_path: &str) -> hmm_ports::PortResult<String> {
            self.texts
                .get(relative_path)
                .cloned()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "missing").into())
        }

        fn sha256_hex(&self, relative_path: &str) -> hmm_ports::PortResult<String> {
            self.hashes
                .get(relative_path)
                .cloned()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "missing").into())
        }
    }
}
