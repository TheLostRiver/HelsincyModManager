use super::*;
use hmm_core::{ContentTransformInvocation, OriginalInstallEvidence};

#[test]
fn verified_origin_allows_only_the_registered_transforms_exact_input_and_output() {
    for case in [
        "approved",
        "unregistered",
        "wrong-input",
        "wrong-output",
        "wrong-file",
    ] {
        let fixture = Fixture::ready();
        let mut manifest = installed_manifest();
        manifest
            .entries
            .retain(|entry| entry.package_file_id.as_str() == "retained");
        manifest.replacement_bindings.clear();
        fixture.manifests.set_manifest(Some(manifest.clone()));
        let original_binding = ReplacementBindingSnapshot::new(
            ReplacementBinding::new(
                ReplacementBindingId::parse("binding-v1").unwrap(),
                ModId::new("mod-a"),
                ProfileId::new("default"),
                ReplacementSourceId::parse("mhw:armor:f_equip:pl121_0000").unwrap(),
                ReplacementTargetId::parse("mhw:armor:guardian-alpha").unwrap(),
                0,
            )
            .unwrap(),
            Some(ModRevisionId::new("v1")),
            "pl121_0000",
            "pl121_0000",
            "pl/f_equip",
            "pl/f_equip",
            ReplacementTargetKind::parse("armor").unwrap(),
        )
        .unwrap();
        let original_plan =
            InstallPlan::from_providers([candidate_provider("content/retained.bin", "retained")])
                .with_replacement_bindings(vec![original_binding])
                .unwrap();
        let original_summary = summarize(b"same");
        let evidence = OriginalInstallEvidence::verify(
            &manifest,
            &ModId::new("mod-a"),
            &ModRevisionId::new("v1"),
            &original_plan,
            &BTreeMap::from([(PackageFileId::new("retained"), original_summary.clone())]),
            &BTreeMap::from([(
                manifest.entries[0].target_path.clone(),
                original_summary.clone(),
            )]),
        )
        .unwrap();
        let candidate_bytes = b"verified transformed material";
        fixture.source.set("retained", candidate_bytes);
        let candidate =
            InstallPlan::from_providers([candidate_provider("content/new-target.bin", "retained")])
                .with_replacement_bindings(vec![ReplacementBindingSnapshot::new(
                    ReplacementBinding::new(
                        ReplacementBindingId::parse("binding-v1").unwrap(),
                        ModId::new("mod-a"),
                        ProfileId::new("default"),
                        ReplacementSourceId::parse("mhw:armor:f_equip:pl121_0000").unwrap(),
                        ReplacementTargetId::parse("mhw:armor:fatalis-alpha").unwrap(),
                        0,
                    )
                    .unwrap(),
                    Some(ModRevisionId::new("v1")),
                    "pl121_0000",
                    "pl129_0000",
                    "pl/f_equip",
                    "pl/f_equip",
                    ReplacementTargetKind::parse("armor").unwrap(),
                )
                .unwrap()])
                .unwrap();
        let transform = ContentTransformInvocation::new(
            1,
            "fixture.material-path",
            1,
            if case == "wrong-input" {
                "a".repeat(64)
            } else {
                original_summary.sha256
            },
            if case == "wrong-output" {
                "b".repeat(64)
            } else {
                summarize(candidate_bytes).sha256
            },
            "c".repeat(64),
            BTreeMap::new(),
            BTreeMap::new(),
        )
        .unwrap();
        let transforms = if case == "unregistered" {
            BTreeMap::new()
        } else {
            BTreeMap::from([(
                PackageFileId::new(if case == "wrong-file" {
                    "another"
                } else {
                    "retained"
                }),
                transform,
            )])
        };
        let mut request = default_request();
        request.candidate_revision_id = ModRevisionId::new("v1");
        let preview = fixture
            .service
            .clone()
            .with_content_transforms(transforms)
            .prepare_replacement_target_switch_with_origin(request, candidate, Some(evidence), None)
            .unwrap()
            .into_preview();
        if case == "approved" {
            assert_eq!(preview.status, ReinstallPreviewStatus::Ready, "{preview:?}");
        } else {
            assert_blocked(&preview, ReinstallBlockingReason::OriginalInstallUnverified);
        }
        fixture.assert_zero_mutations();
    }
}
