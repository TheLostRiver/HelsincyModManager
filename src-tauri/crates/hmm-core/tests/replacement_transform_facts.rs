use hmm_core::{
    ContentTransformInvocation, ContentTransformerIdentity, FileLayer, GameId, InstallTargetPath,
    ModId, ModRevisionId, PackageFileId, ProfileId, ReplacementAdapterFacts, ReplacementBinding,
    ReplacementBindingId, ReplacementBindingSnapshot, ReplacementSource, ReplacementSourceId,
    ReplacementTargetId, ReplacementTargetKind, RetargetAction, RetargetPlan,
};
use std::collections::BTreeMap;

fn target(path: &str) -> InstallTargetPath {
    InstallTargetPath::parse(path, ["nativePC"]).expect("target")
}

fn digest(byte: char) -> String {
    byte.to_string().repeat(64)
}

fn binding() -> ReplacementBinding {
    ReplacementBinding::new(
        ReplacementBindingId::parse("binding-weapon").expect("binding id"),
        ModId::new("mod-weapon"),
        ProfileId::new("default"),
        ReplacementSourceId::parse("mhw:weapon:source").expect("source id"),
        ReplacementTargetId::parse("mhw:weapon:target").expect("target id"),
        42,
    )
    .expect("binding")
}

fn source() -> ReplacementSource {
    ReplacementSource::new(
        ReplacementSourceId::parse("mhw:weapon:source").expect("source id"),
        GameId::mhw(),
        ReplacementTargetKind::parse("weapon").expect("kind"),
        "one001",
        "wp/one",
        true,
    )
    .expect("source")
}

fn invocation() -> ContentTransformInvocation {
    ContentTransformInvocation::new(
        1,
        "mhw.weapon.mrl3-texture-path.v1",
        1,
        digest('a'),
        digest('b'),
        digest('c'),
        BTreeMap::from([(PackageFileId::new("pair.mod3"), digest('d'))]),
        BTreeMap::from([
            (
                "source_relative_path".to_owned(),
                "nativePC/wp/one/one001/mod/one001.mrl3".to_owned(),
            ),
            (
                "companion_relative_path".to_owned(),
                "nativePC/wp/one/one001/mod/one001.mod3".to_owned(),
            ),
            ("target_main_id".to_owned(), "one002".to_owned()),
        ]),
    )
    .expect("invocation")
}

fn transformed_plan() -> RetargetPlan {
    let action = RetargetAction::new(
        PackageFileId::new("pair.mrl3"),
        target("nativePC/wp/one/one001/mod/one001.mrl3"),
        target("nativePC/wp/one/one002/mod/one002.mrl3"),
        ReplacementSourceId::parse("mhw:weapon:source").expect("source id"),
        "one001",
        "one002",
        "wp/one",
        "wp/one",
    )
    .expect("action")
    .with_content_transform(invocation());
    RetargetPlan::new(binding(), source(), vec![action], Vec::new()).expect("plan")
}

#[test]
fn legacy_snapshot_omits_optional_adapter_facts() {
    let snapshot = ReplacementBindingSnapshot::new(
        binding(),
        Some(ModRevisionId::new("revision-v1")),
        "one001",
        "one002",
        "wp/one",
        "wp/one",
        ReplacementTargetKind::parse("weapon").expect("kind"),
    )
    .expect("snapshot");

    let json = serde_json::to_string(&snapshot).expect("serialize");
    assert!(!json.contains("adapter_facts"));
    let restored: ReplacementBindingSnapshot = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.adapter_facts(), None);
}

#[test]
fn transformed_plan_requires_matching_adapter_transform_set_digest() {
    let plan = transformed_plan();
    assert!(plan.validate_transform_facts().is_err());
    let transform_set_digest = plan.content_transform_set_sha256();
    let facts = ReplacementAdapterFacts::new(
        1,
        "mhw.weapon",
        "mrl3-texture-path",
        1,
        digest('e'),
        digest('f'),
        transform_set_digest,
    )
    .expect("adapter facts")
    .with_transformers(
        vec![
            ContentTransformerIdentity::new("mhw.weapon.mrl3-texture-path.v1", 1)
                .expect("transformer identity"),
        ],
        1,
        1,
    )
    .expect("transformer facts");
    let plan = plan.with_adapter_facts(facts).expect("sealed plan");
    plan.validate_transform_facts().expect("matching facts");

    let snapshot = ReplacementBindingSnapshot::from_retarget_plan(
        &plan,
        Some(ModRevisionId::new("revision-v1")),
    );
    assert_eq!(snapshot.adapter_facts(), plan.adapter_facts());
    let json = serde_json::to_string(&snapshot).expect("serialize transformed snapshot");
    assert!(json.contains("mhw.weapon.mrl3-texture-path.v1"));
    assert!(json.contains("\"part_count\":1"));
    assert!(json.contains("\"file_count\":1"));
    assert_eq!(FileLayer::new("base", 0).priority, 0);
}

#[test]
fn transformed_plan_rejects_identity_or_count_drift() {
    let plan = transformed_plan();
    let transform_set_sha256 = plan.content_transform_set_sha256();
    let build = |identity: &str, file_count| {
        ReplacementAdapterFacts::new(
            1,
            "mhw.weapon",
            "mrl3-texture-path",
            1,
            digest('e'),
            digest('f'),
            transform_set_sha256.clone(),
        )
        .expect("adapter facts")
        .with_transformers(
            vec![ContentTransformerIdentity::new(identity, 1).expect("identity")],
            1,
            file_count,
        )
        .expect("transformer facts")
    };

    assert!(plan
        .clone()
        .with_adapter_facts(build("mhw.weapon.other.v1", 1))
        .is_err());
    assert!(plan
        .with_adapter_facts(build("mhw.weapon.mrl3-texture-path.v1", 2))
        .is_err());
}

#[test]
fn invocation_rejects_unbounded_or_noncanonical_facts() {
    let invalid_digest = ContentTransformInvocation::new(
        1,
        "mhw.weapon.mrl3-texture-path.v1",
        1,
        "SHA256:NOT-CANONICAL",
        digest('b'),
        digest('c'),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    assert!(invalid_digest.is_err());

    let too_many_parameters = (0..17)
        .map(|index| (format!("key_{index}"), "value".to_owned()))
        .collect();
    let unbounded = ContentTransformInvocation::new(
        1,
        "mhw.weapon.mrl3-texture-path.v1",
        1,
        digest('a'),
        digest('b'),
        digest('c'),
        BTreeMap::new(),
        too_many_parameters,
    );
    assert!(unbounded.is_err());
}
