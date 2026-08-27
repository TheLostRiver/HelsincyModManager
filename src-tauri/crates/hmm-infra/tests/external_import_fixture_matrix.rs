use hmm_core::{
    ExternalImportBatchId, ExternalImportCandidate, ExternalImportCandidateId,
    ExternalImportCandidateStatus, ExternalImportConflictKind, ExternalImportConflictResolution,
    ExternalImportMetadataHint, ExternalImportReasonCode, ExternalImportResourceBudget,
    ExternalImportResourceUsage, ExternalImportSelection, ExternalImportSelectionDecision,
    ExternalImportSelectionId, ExternalImportSelectionMutation,
};
use std::fs;
use std::path::{Path, PathBuf};

const SAFE_INFO_XML: &str =
    "<mod><moduleName>Fixture Mod</moduleName><author>Fixture Author</author></mod>";
const DTD_INFO_XML: &str =
    "<!DOCTYPE mod [<!ENTITY xxe SYSTEM \"file:///not-a-real-path\">]><mod/>";

#[derive(Clone, Copy)]
enum FixtureLayout {
    Ready,
    NonNumericDirectory,
    MissingFilesDirectory,
    MissingInfoXml,
    NestedNumericDirectory,
    UnsupportedSpecialEntry,
    MetadataInvalid,
    ResourceLimitExceeded,
    SourceUnreadable,
}

struct FixtureCase {
    name: &'static str,
    layout: FixtureLayout,
    expected_status: ExternalImportCandidateStatus,
    expected_reason: Option<ExternalImportReasonCode>,
}

const FIXTURE_CASES: &[FixtureCase] = &[
    FixtureCase {
        name: "ready_numeric_root",
        layout: FixtureLayout::Ready,
        expected_status: ExternalImportCandidateStatus::Ready,
        expected_reason: None,
    },
    FixtureCase {
        name: "non_numeric_root",
        layout: FixtureLayout::NonNumericDirectory,
        expected_status: ExternalImportCandidateStatus::StructureInvalid,
        expected_reason: Some(ExternalImportReasonCode::StructureInvalid),
    },
    FixtureCase {
        // 元数据在、载荷不在盒子库中(狩技盒子「无操作」安装方式的残留):
        // 单独归为缺载荷,预览可以带 mod 名明确标注,不与结构无效混同。
        name: "missing_files",
        layout: FixtureLayout::MissingFilesDirectory,
        expected_status: ExternalImportCandidateStatus::PayloadMissing,
        expected_reason: Some(ExternalImportReasonCode::PayloadMissing),
    },
    FixtureCase {
        name: "missing_info_xml",
        layout: FixtureLayout::MissingInfoXml,
        expected_status: ExternalImportCandidateStatus::StructureInvalid,
        expected_reason: Some(ExternalImportReasonCode::StructureInvalid),
    },
    FixtureCase {
        name: "nested_numeric_directory",
        layout: FixtureLayout::NestedNumericDirectory,
        expected_status: ExternalImportCandidateStatus::StructureInvalid,
        expected_reason: Some(ExternalImportReasonCode::StructureInvalid),
    },
    FixtureCase {
        name: "unsupported_special_entry",
        layout: FixtureLayout::UnsupportedSpecialEntry,
        expected_status: ExternalImportCandidateStatus::UnsupportedEntry,
        expected_reason: Some(ExternalImportReasonCode::UnsupportedEntry),
    },
    FixtureCase {
        name: "doctype_metadata",
        layout: FixtureLayout::MetadataInvalid,
        expected_status: ExternalImportCandidateStatus::MetadataInvalid,
        expected_reason: Some(ExternalImportReasonCode::MetadataInvalid),
    },
    FixtureCase {
        name: "resource_budget",
        layout: FixtureLayout::ResourceLimitExceeded,
        expected_status: ExternalImportCandidateStatus::ResourceLimitExceeded,
        expected_reason: Some(ExternalImportReasonCode::ResourceLimitExceeded),
    },
    FixtureCase {
        name: "unreadable_source",
        layout: FixtureLayout::SourceUnreadable,
        expected_status: ExternalImportCandidateStatus::SourceUnreadable,
        expected_reason: Some(ExternalImportReasonCode::SourceUnreadable),
    },
];

#[test]
fn external_import_fixture_matrix_covers_handcrafted_rejection_contracts() {
    let reasons = FIXTURE_CASES
        .iter()
        .filter_map(|case| case.expected_reason)
        .collect::<Vec<_>>();

    assert!(reasons.contains(&ExternalImportReasonCode::StructureInvalid));
    assert!(reasons.contains(&ExternalImportReasonCode::MetadataInvalid));
    assert!(reasons.contains(&ExternalImportReasonCode::UnsupportedEntry));
    assert!(reasons.contains(&ExternalImportReasonCode::ResourceLimitExceeded));
    assert!(reasons.contains(&ExternalImportReasonCode::SourceUnreadable));
    assert!(reasons.contains(&ExternalImportReasonCode::PayloadMissing));
    for case in FIXTURE_CASES {
        assert_eq!(case.expected_status.reason_code(), case.expected_reason);
        assert!(!case.name.contains("hunting-box"));
    }
}

#[test]
fn external_import_fixture_matrix_uses_temporary_handcrafted_inputs_and_selection_is_read_only() {
    for case in FIXTURE_CASES {
        let temp = tempfile::tempdir().expect("temporary fixture directory");
        let fixture_root = temp.path().join("external-import-fixture");
        let source_item = write_fixture(&fixture_root, case.layout);
        let snapshot_before = snapshot_tree(&fixture_root);

        let candidate = ExternalImportCandidate {
            batch_id: ExternalImportBatchId::new("batch-fixture"),
            candidate_id: ExternalImportCandidateId::new(case.name),
            source_item_key_hash: format!("item-key-hash-{}", case.name),
            content_fingerprint: format!("content-fingerprint-{}", case.name),
            metadata_hint: ExternalImportMetadataHint::default(),
            resource_usage: ExternalImportResourceUsage {
                file_count: 1,
                source_bytes: 1,
                materialization_bytes: 1,
            },
            preview_status: case.expected_status,
            conflict_kind: ExternalImportConflictKind::None,
        };
        let mut selection = ExternalImportSelection::new(
            ExternalImportSelectionId::new(format!("selection-{}", case.name)),
            ExternalImportBatchId::new("batch-fixture"),
            1_000,
        );
        let mutation = ExternalImportSelectionMutation {
            candidate_id: candidate.candidate_id.clone(),
            selected: true,
            decision: (case.expected_status == ExternalImportCandidateStatus::MetadataInvalid)
                .then_some(ExternalImportSelectionDecision {
                    conflict_resolution: Some(
                        ExternalImportConflictResolution::IgnoreInvalidMetadata,
                    ),
                    category_id: None,
                }),
        };
        let outcome = selection.apply_mutation(
            0,
            &[mutation],
            &[candidate],
            &ExternalImportResourceBudget::default(),
            1,
        );

        match case.expected_status {
            ExternalImportCandidateStatus::Ready
            | ExternalImportCandidateStatus::MetadataInvalid => {
                outcome.expect("selectable artificial fixture status succeeds");
            }
            _ => assert!(outcome.is_err(), "{} must remain blocked", case.name),
        }
        assert_eq!(snapshot_tree(&fixture_root), snapshot_before);
        assert!(source_item.starts_with(&fixture_root));
    }
}

fn write_fixture(root: &Path, layout: FixtureLayout) -> PathBuf {
    fs::create_dir_all(root).expect("create temporary fixture root");
    let candidate_root = match layout {
        FixtureLayout::NonNumericDirectory => root.join("not-a-number"),
        FixtureLayout::NestedNumericDirectory => root.join("container").join("1002"),
        _ => root.join("1001"),
    };
    fs::create_dir_all(&candidate_root).expect("create candidate root");

    match layout {
        FixtureLayout::MissingFilesDirectory => {
            fs::write(candidate_root.join("info.xml"), SAFE_INFO_XML)
                .expect("write artificial XML");
        }
        FixtureLayout::MissingInfoXml => {
            fs::create_dir_all(candidate_root.join("files")).expect("create files directory");
            fs::write(candidate_root.join("files").join("fixture.bin"), b"fixture")
                .expect("write artificial file");
        }
        FixtureLayout::UnsupportedSpecialEntry => {
            fs::create_dir_all(candidate_root.join("files")).expect("create files directory");
            // This marker represents a special entry without requiring symlink privileges in CI.
            fs::write(
                candidate_root.join("files").join("special-entry.marker"),
                b"fixture",
            )
            .expect("write artificial marker");
            fs::write(candidate_root.join("info.xml"), SAFE_INFO_XML)
                .expect("write artificial XML");
        }
        FixtureLayout::MetadataInvalid => {
            fs::create_dir_all(candidate_root.join("files")).expect("create files directory");
            fs::write(candidate_root.join("files").join("fixture.bin"), b"fixture")
                .expect("write artificial file");
            fs::write(candidate_root.join("info.xml"), DTD_INFO_XML).expect("write artificial XML");
        }
        _ => {
            fs::create_dir_all(candidate_root.join("files")).expect("create files directory");
            fs::write(candidate_root.join("files").join("fixture.bin"), b"fixture")
                .expect("write artificial file");
            fs::write(candidate_root.join("info.xml"), SAFE_INFO_XML)
                .expect("write artificial XML");
        }
    }

    candidate_root
}

fn snapshot_tree(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut entries = Vec::new();
    collect_snapshot(root, root, &mut entries);
    entries.sort();
    entries
}

fn collect_snapshot(root: &Path, directory: &Path, entries: &mut Vec<(String, Vec<u8>)>) {
    for entry in fs::read_dir(directory).expect("read artificial fixture directory") {
        let entry = entry.expect("read artificial fixture entry");
        let path = entry.path();
        if path.is_dir() {
            collect_snapshot(root, &path, entries);
        } else {
            let relative = path
                .strip_prefix(root)
                .expect("fixture entry remains under temporary root")
                .to_string_lossy()
                .replace('\\', "/");
            entries.push((
                relative,
                fs::read(path).expect("read artificial fixture file"),
            ));
        }
    }
}
