use super::*;
use serde_json::{json, Value};

fn fixture() -> Value {
    json!({
        "schema_version":1,"catalog_version":"mhw-kinsect-v1","game_id":"mhw",
        "targets":[{
            "stable_id":generate_mhw_equipment_stable_id(EquipmentCandidateTargetKind::Kinsect,"wp/mus","nativePC/wp/mus/mus001").unwrap(),
            "target_type":"kinsect","resource_path":"nativePC/wp/mus/mus001","internal_id":"mus001",
            "metadata":{"path_family":"wp/mus"},"status":"active",
            "names":{
                "en":{"display_name":"Artificial I","aliases":["Artificial II"]},
                "zh_cn":{"display_name":"人工一号","aliases":["人工二号"]},
                "ja":{"display_name":"人工一","aliases":["人工二"]}
            }
        }]
    })
}

#[test]
fn catalog_keeps_every_localized_alias_and_only_exposes_active_targets() {
    let mut input = fixture();
    let targets = parse_targets(&input.to_string()).unwrap();
    assert_eq!(
        targets[0].localized_aliases().unwrap()["en"],
        ["Artificial II"]
    );
    assert_eq!(targets[0].aliases().len(), 3);
    input["targets"][0]["status"] = json!("hidden");
    assert!(parse_targets(&input.to_string()).unwrap().is_empty());
    input["targets"][0]["status"] = json!("dummy");
    assert!(parse_targets(&input.to_string()).is_err());
}

#[test]
fn catalog_rejects_forged_identity_metadata_duplicate_targets_and_missing_locales() {
    for (key, value) in [
        ("stable_id", "mhw:kinsect:forged"),
        ("target_type", "weapon"),
        ("resource_path", "nativePC/wp/mus/mus002"),
        ("internal_id", "rod001"),
    ] {
        let mut input = fixture();
        input["targets"][0][key] = json!(value);
        assert!(parse_targets(&input.to_string()).is_err(), "{key}");
    }
    let mut input = fixture();
    input["targets"][0]["metadata"]["path_family"] = json!("wp/rod");
    assert!(parse_targets(&input.to_string()).is_err());
    let mut input = fixture();
    input["targets"][0]["names"]
        .as_object_mut()
        .unwrap()
        .remove("ja");
    assert!(parse_targets(&input.to_string()).is_err());
    let mut input = fixture();
    let duplicate = input["targets"][0].clone();
    input["targets"].as_array_mut().unwrap().push(duplicate);
    assert!(parse_targets(&input.to_string()).is_err());
}
