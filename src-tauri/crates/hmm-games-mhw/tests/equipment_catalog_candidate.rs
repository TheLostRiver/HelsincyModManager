use hmm_games_mhw::{
    generate_mhw_equipment_stable_id, validate_mhw_equipment_candidate_catalog,
    validate_mhw_equipment_candidate_catalog_for_bundling, EquipmentCandidateBundlingError,
    EquipmentCandidateTargetKind, MHW_EQUIPMENT_CANDIDATE_JSON_SCHEMA,
    MHW_EQUIPMENT_CANDIDATE_SCHEMA_VERSION,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

const SOURCE_ID: &str = "fixture-source";

fn source(license_status: &str) -> Value {
    let license = match license_status {
        "redistributable" => json!({
            "status": "redistributable",
            "spdx_expression": "CC0-1.0",
            "evidence_url": "https://example.invalid/license",
            "attribution": "Artificial test fixture",
            "reviewed_by": "fixture-reviewer",
            "reviewed_at": "2026-08-05"
        }),
        status => json!({ "status": status }),
    };

    json!({
        "source_id": SOURCE_ID,
        "source_name": "Artificial test fixture",
        "source_url": "https://example.invalid/catalog",
        "retrieved_at": "2026-08-05",
        "license": license
    })
}

fn target(
    kind: EquipmentCandidateTargetKind,
    path_family: &str,
    resource_path: &str,
    display_name: &str,
    status: &str,
) -> Value {
    let kind_name = match kind {
        EquipmentCandidateTargetKind::Armor => "armor",
        EquipmentCandidateTargetKind::Weapon => "weapon",
    };
    let stable_id = generate_mhw_equipment_stable_id(kind, path_family, resource_path)
        .expect("test target identity should be valid");

    json!({
        "stable_id": stable_id,
        "target_kind": kind_name,
        "path_family": path_family,
        "resource_path": resource_path,
        "status": status,
        "names": {
            "en": {
                "display_name": display_name,
                "aliases": []
            }
        },
        "source_ids": [SOURCE_ID],
        "legacy_ids": []
    })
}

fn catalog(sources: Vec<Value>, targets: Vec<Value>) -> String {
    json!({
        "schema_version": MHW_EQUIPMENT_CANDIDATE_SCHEMA_VERSION,
        "catalog_version": "artificial-candidate-v1",
        "game_id": "mhw",
        "sources": sources,
        "targets": targets
    })
    .to_string()
}

fn issue_codes(source: &str) -> BTreeSet<String> {
    validate_mhw_equipment_candidate_catalog(source)
        .expect("candidate JSON should parse")
        .issues
        .into_iter()
        .map(|issue| issue.code)
        .collect()
}

#[test]
fn candidate_schema_is_valid_json_and_locks_v1() {
    let schema: Value =
        serde_json::from_str(MHW_EQUIPMENT_CANDIDATE_JSON_SCHEMA).expect("JSON schema");

    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(
        schema["properties"]["schema_version"]["const"],
        MHW_EQUIPMENT_CANDIDATE_SCHEMA_VERSION
    );
    assert_eq!(schema["properties"]["game_id"]["const"], "mhw");
}

#[test]
fn validates_active_and_hidden_targets_with_complete_redistribution_evidence() {
    let mut armor = target(
        EquipmentCandidateTargetKind::Armor,
        "pl/f_equip",
        "nativePC/pl/f_equip/pl900_0000",
        "Artificial Armor",
        "active",
    );
    armor["legacy_ids"] = json!(["mhw:armor:fatalis-alpha"]);
    let weapon = target(
        EquipmentCandidateTargetKind::Weapon,
        "wp/test_family",
        "nativePC/wp/test_family/wp900_0000",
        "Artificial Weapon",
        "hidden",
    );
    let source = catalog(vec![source("redistributable")], vec![armor, weapon]);

    let report = validate_mhw_equipment_candidate_catalog_for_bundling(&source)
        .expect("valid candidate should pass the bundling gate");

    assert!(report.valid);
    assert!(report.bundled_eligible);
    assert_eq!(report.target_count, 2);
    assert_eq!(report.active_target_count, 1);
    assert_eq!(report.hidden_target_count, 1);
    assert_eq!(report.dummy_target_count, 0);
}

#[test]
fn stable_id_depends_on_normalized_resource_identity_not_display_data_or_case() {
    let lower = generate_mhw_equipment_stable_id(
        EquipmentCandidateTargetKind::Armor,
        "pl/f_equip",
        "nativePC/pl/f_equip/pl901_0000",
    )
    .expect("lowercase identity");
    let mixed_case = generate_mhw_equipment_stable_id(
        EquipmentCandidateTargetKind::Armor,
        "pl/f_equip",
        "nativePC/PL/F_EQUIP/pl901_0000",
    )
    .expect("mixed-case identity");

    assert_eq!(lower, mixed_case);
    assert!(lower.starts_with("mhw:armor:"));
    assert_eq!(lower.len(), "mhw:armor:".len() + 64);
}

#[test]
fn unknown_license_is_valid_for_audit_but_blocked_for_bundling() {
    let candidate = catalog(
        vec![source("unknown")],
        vec![target(
            EquipmentCandidateTargetKind::Armor,
            "pl/f_equip",
            "nativePC/pl/f_equip/pl902_0000",
            "Unknown License Armor",
            "active",
        )],
    );

    let report =
        validate_mhw_equipment_candidate_catalog(&candidate).expect("candidate should parse");
    assert!(report.valid);
    assert!(!report.bundled_eligible);
    assert_eq!(report.bundle_blockers[0].code, "license_unknown");
    assert_eq!(
        validate_mhw_equipment_candidate_catalog_for_bundling(&candidate),
        Err(EquipmentCandidateBundlingError::EligibilityBlocked)
    );
}

#[test]
fn dummy_target_is_valid_for_audit_but_blocked_for_bundling() {
    let candidate = catalog(
        vec![source("redistributable")],
        vec![target(
            EquipmentCandidateTargetKind::Armor,
            "pl/f_equip",
            "nativePC/pl/f_equip/pl903_0000",
            "Dummy Armor",
            "dummy",
        )],
    );

    let report =
        validate_mhw_equipment_candidate_catalog(&candidate).expect("candidate should parse");
    assert!(report.valid);
    assert!(!report.bundled_eligible);
    assert_eq!(report.dummy_target_count, 1);
    assert_eq!(report.bundle_blockers[0].code, "dummy_target");
}

#[test]
fn rejects_absolute_and_parent_traversal_resource_paths() {
    let unsafe_paths = [
        "/nativePC/pl/f_equip/pl904_0000",
        "C:/nativePC/pl/f_equip/pl904_0000",
        r"\\server\share\nativePC\pl\f_equip\pl904_0000",
        "nativePC/pl/f_equip/../pl904_0000",
    ];

    for resource_path in unsafe_paths {
        let mut invalid = target(
            EquipmentCandidateTargetKind::Armor,
            "pl/f_equip",
            "nativePC/pl/f_equip/pl904_0000",
            "Unsafe Armor",
            "active",
        );
        invalid["resource_path"] = json!(resource_path);
        let candidate = catalog(vec![source("redistributable")], vec![invalid]);

        assert!(issue_codes(&candidate).contains("unsafe_resource_path"));
    }
}

#[test]
fn rejects_case_insensitive_resource_path_collision() {
    let first = target(
        EquipmentCandidateTargetKind::Weapon,
        "wp/test_family",
        "nativePC/wp/test_family/wp905_0000",
        "First Weapon",
        "active",
    );
    let second = target(
        EquipmentCandidateTargetKind::Weapon,
        "wp/test_family",
        "nativePC/wp/test_family/WP905_0000",
        "Second Weapon",
        "active",
    );
    let candidate = catalog(vec![source("redistributable")], vec![first, second]);

    let codes = issue_codes(&candidate);
    assert!(codes.contains("case_insensitive_path_collision"));
    assert!(codes.contains("duplicate_stable_id"));
}

#[test]
fn rejects_duplicate_stable_id_even_when_resource_paths_differ() {
    let first = target(
        EquipmentCandidateTargetKind::Weapon,
        "wp/test_family",
        "nativePC/wp/test_family/wp906_0000",
        "First Stable ID",
        "active",
    );
    let mut second = target(
        EquipmentCandidateTargetKind::Weapon,
        "wp/test_family",
        "nativePC/wp/test_family/wp907_0000",
        "Second Stable ID",
        "active",
    );
    second["stable_id"] = first["stable_id"].clone();
    let candidate = catalog(vec![source("redistributable")], vec![first, second]);

    let codes = issue_codes(&candidate);
    assert!(codes.contains("duplicate_stable_id"));
    assert!(codes.contains("stable_id_mismatch"));
}

#[test]
fn rejects_duplicate_display_name_after_unicode_search_normalization() {
    let first = target(
        EquipmentCandidateTargetKind::Armor,
        "pl/f_equip",
        "nativePC/pl/f_equip/pl908_0000",
        "ＦＩＸＴＵＲＥ ARMOR",
        "active",
    );
    let second = target(
        EquipmentCandidateTargetKind::Armor,
        "pl/f_equip",
        "nativePC/pl/f_equip/pl909_0000",
        "fixture armor",
        "active",
    );
    let candidate = catalog(vec![source("redistributable")], vec![first, second]);

    assert!(issue_codes(&candidate).contains("duplicate_display_name"));
}

#[test]
fn rejects_target_kind_and_path_family_mismatch() {
    let mut invalid = target(
        EquipmentCandidateTargetKind::Armor,
        "pl/f_equip",
        "nativePC/pl/f_equip/pl910_0000",
        "Wrong Family Armor",
        "active",
    );
    invalid["path_family"] = json!("wp/test_family");
    let candidate = catalog(vec![source("redistributable")], vec![invalid]);

    assert!(issue_codes(&candidate).contains("wrong_path_family"));
}

#[test]
fn rejects_redistributable_status_without_complete_license_evidence() {
    let mut incomplete_source = source("redistributable");
    incomplete_source["license"]
        .as_object_mut()
        .expect("license object")
        .remove("spdx_expression");
    let candidate = catalog(
        vec![incomplete_source],
        vec![target(
            EquipmentCandidateTargetKind::Armor,
            "pl/f_equip",
            "nativePC/pl/f_equip/pl911_0000",
            "Incomplete License Armor",
            "active",
        )],
    );

    let report =
        validate_mhw_equipment_candidate_catalog(&candidate).expect("candidate should parse");
    assert!(!report.valid);
    assert!(!report.bundled_eligible);
    assert!(report
        .issues
        .iter()
        .any(|issue| issue.code == "incomplete_redistributable_license"));
}

#[test]
fn validation_report_does_not_echo_untrusted_candidate_values() {
    let mut invalid = target(
        EquipmentCandidateTargetKind::Armor,
        "pl/f_equip",
        "nativePC/pl/f_equip/pl912_0000",
        "Sensitive Display Value",
        "active",
    );
    invalid["resource_path"] = json!("C:/Users/Sensitive/resource");
    invalid["names"] = json!({
        "C:/Users/Sensitive": {
            "display_name": "Sensitive Display Value",
            "aliases": ["Sensitive Alias Value"]
        }
    });
    let mut candidate: Value =
        serde_json::from_str(&catalog(vec![source("redistributable")], vec![invalid]))
            .expect("candidate value");
    candidate["catalog_version"] = json!("C:/Users/Sensitive/catalog");

    let report = validate_mhw_equipment_candidate_catalog(&candidate.to_string())
        .expect("candidate should parse");
    let output = serde_json::to_string(&report).expect("validation report");

    assert_eq!(report.catalog_version, "<invalid>");
    assert!(!output.contains("Users"));
    assert!(!output.contains("Sensitive"));
}

#[test]
fn rejects_nonexistent_license_review_date() {
    let mut invalid_source = source("redistributable");
    invalid_source["license"]["reviewed_at"] = json!("2026-02-30");
    let candidate = catalog(
        vec![invalid_source],
        vec![target(
            EquipmentCandidateTargetKind::Armor,
            "pl/f_equip",
            "nativePC/pl/f_equip/pl913_0000",
            "Invalid Review Date Armor",
            "active",
        )],
    );

    assert!(issue_codes(&candidate).contains("incomplete_redistributable_license"));
}
