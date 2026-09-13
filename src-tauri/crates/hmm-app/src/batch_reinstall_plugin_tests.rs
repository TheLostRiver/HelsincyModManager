use super::*;
use hmm_core::{
    GameId, PluginFileChoice, PluginFileChoiceKind, PluginSelectionScope, PluginSelectionSnapshot,
};

fn plugin(
    mod_id: &str,
    revision: &str,
    bytes: &[u8],
    choice: PluginFileChoiceKind,
) -> PluginSelectionSnapshot {
    PluginSelectionSnapshot::new(
        PluginSelectionScope {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new(mod_id),
            revision_id: ModRevisionId::new(revision),
        },
        "fixture.plugins",
        1,
        vec![PluginFileChoice {
            package_file_id: PackageFileId::new("plugin"),
            target_path: InstallTargetPath::parse("content/plugin.dll", ["content"]).unwrap(),
            source_file: summary(bytes),
            choice,
            excluded_by_package_selection: false,
        }],
    )
    .unwrap()
}

#[test]
fn batch_digest_binds_excluded_plugin_bytes_choices_and_only_this_mods_applied_facts() {
    let fixture = Fixture::ready();
    let mut prepared = fixture.prepare(default_request());
    let no_plugins = prepared.batch_plan_digest();
    let excluded = plugin("mod-a", "v2", b"excluded", PluginFileChoiceKind::Exclude);
    prepared.candidate_plugin_selections = vec![excluded.clone()];
    let candidate_digest = prepared.batch_plan_digest();
    assert_ne!(no_plugins, candidate_digest);
    for changed in [
        plugin("mod-a", "v2", b"changed!", PluginFileChoiceKind::Exclude),
        plugin("mod-a", "v2", b"excluded", PluginFileChoiceKind::Include),
    ] {
        prepared.candidate_plugin_selections = vec![changed];
        assert_ne!(candidate_digest, prepared.batch_plan_digest());
    }
    prepared.candidate_plugin_selections = vec![excluded];
    prepared.old_manifest.plugin_selections = vec![plugin(
        "mod-b",
        "other",
        b"unrelated",
        PluginFileChoiceKind::Exclude,
    )];
    assert_eq!(candidate_digest, prepared.batch_plan_digest());
    prepared.old_manifest.plugin_selections.push(plugin(
        "mod-a",
        "v1",
        b"old",
        PluginFileChoiceKind::Exclude,
    ));
    let applied_digest = prepared.batch_plan_digest();
    assert_ne!(candidate_digest, applied_digest);
    prepared.old_manifest.plugin_selections[1] =
        plugin("mod-a", "v1", b"drifted", PluginFileChoiceKind::Exclude);
    assert_ne!(applied_digest, prepared.batch_plan_digest());
    prepared.old_manifest.plugin_selections.clear();
    prepared.candidate_plugin_selections.clear();
    assert_eq!(no_plugins, prepared.batch_plan_digest());
    fixture.assert_zero_mutations();
}

#[test]
fn batch_accepts_generated_identity_bindings_only_for_revision_upgrades() {
    let fixture = Fixture::ready();
    let mut prepared = fixture.prepare(default_request());
    let input = ReinstallBatchItemInput {
        intent: Default::default(),
        mod_id: ModId::new("mod-a"),
        installed_revision_id: ModRevisionId::new("v1"),
        candidate_revision_id: ModRevisionId::new("v2"),
        layer: FileLayer::new("base", 0),
        replacement_binding_snapshot: None,
    };
    assert_eq!(
        prepared.batch_item_facts(&input).blocking_reasons,
        ["replacement_binding_changed"]
    );
    prepared.candidate_replacement_bindings = ["pl121_0000", "pl122_0000"]
        .into_iter()
        .map(|id| {
            ReplacementBindingSnapshot::new(
                ReplacementBinding::new(
                    ReplacementBindingId::parse(format!("identity-{id}")).unwrap(),
                    input.mod_id.clone(),
                    ProfileId::new("default"),
                    ReplacementSourceId::parse(format!("mhw:armor:f_equip:{id}")).unwrap(),
                    ReplacementTargetId::parse(format!("mhw:armor:f_equip:{id}")).unwrap(),
                    0,
                )
                .unwrap(),
                Some(input.candidate_revision_id.clone()),
                id,
                id,
                "pl/f_equip",
                "pl/f_equip",
                ReplacementTargetKind::parse("armor").unwrap(),
            )
            .unwrap()
        })
        .collect();
    assert!(prepared
        .batch_item_facts(&input)
        .blocking_reasons
        .is_empty());
    let generated_digest = prepared.batch_plan_digest();
    prepared.candidate_replacement_bindings.pop();
    assert_ne!(generated_digest, prepared.batch_plan_digest());
    prepared.installed_revision_id = input.candidate_revision_id.clone();
    let same_revision = ReinstallBatchItemInput {
        installed_revision_id: input.candidate_revision_id.clone(),
        ..input
    };
    assert_eq!(
        prepared.batch_item_facts(&same_revision).blocking_reasons,
        ["replacement_binding_changed"]
    );
    fixture.assert_zero_mutations();
}
