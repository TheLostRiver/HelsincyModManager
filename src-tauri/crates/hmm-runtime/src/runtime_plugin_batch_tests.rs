use super::*;
use hmm_core::{
    BatchExecutionPolicy, BatchItemInput, BatchOperation, BatchPlanRequest, BatchPlanStatus,
    InstallBatchItemInput, ReinstallBatchItemInput, ReinstallIntent, BATCH_PLAN_SCHEMA_VERSION,
};

fn request(
    fixture: &AutomationFixture,
    revision: ModRevisionId,
    upgrade: bool,
) -> crate::BatchLifecyclePlanRequest {
    let scope = fixture.scope();
    BatchPlanRequest {
        schema_version: BATCH_PLAN_SCHEMA_VERSION,
        operation: if upgrade {
            BatchOperation::Reinstall
        } else {
            BatchOperation::Install
        },
        game_id: GameId::mhw(),
        profile_id: scope.profile_id,
        execution_policy: BatchExecutionPolicy::StopOnFailure,
        items: vec![if upgrade {
            BatchItemInput::Reinstall(ReinstallBatchItemInput {
                intent: ReinstallIntent::Standard,
                mod_id: fixture.mod_id.clone(),
                installed_revision_id: scope.revision_id,
                candidate_revision_id: revision,
                layer: FileLayer::new("base", 0),
                replacement_binding_snapshot: None,
            })
        } else {
            BatchItemInput::Install(InstallBatchItemInput {
                mod_id: fixture.mod_id.clone(),
                revision_id: revision,
                layer: FileLayer::new("base", 0),
                replacement_binding_snapshot: None,
            })
        }],
    }
    .into()
}

#[test]
fn batch_preview_and_sealed_execution_reject_changed_excluded_plugin_bytes() {
    for upgrade in [false, true] {
        for after_seal in [false, true] {
            let fixture = AutomationFixture::new();
            let revision = if upgrade {
                fixture.install();
                let archive = fixture._temp.path().join("excluded-candidate.zip");
                create_fixture_zip(
                    &archive,
                    &[(MODEL, b"new model"), (PLUGIN, b"unsupported dll A")],
                );
                import_candidate_fixture_revision(
                    &fixture.state,
                    &archive,
                    &fixture.mod_id,
                    &fixture.scope().revision_id,
                )
                .1
            } else {
                fs::write(fixture.package().join(PLUGIN), b"unsupported dll A").unwrap();
                fixture.scope().revision_id
            };
            let request = request(&fixture, revision.clone(), upgrade);
            let scope = hmm_core::PluginSelectionScope {
                revision_id: revision,
                ..fixture.scope()
            };
            let original = fixture
                .state
                .plugin_selection
                .inventory(&scope)
                .unwrap()
                .unwrap();
            assert!(!original.candidates[0].selected);
            let preview = crate::BatchLifecycleAutomation::preview_request(
                &fixture.environment,
                request.clone(),
            )
            .unwrap();
            assert_eq!(preview.plan.status(), BatchPlanStatus::Ready, "{preview:?}");
            let database = fixture.state.database_handle();
            let sealed = after_seal.then(|| {
                crate::BatchLifecycleAutomation::seal_request_with_database(
                    &fixture.environment,
                    request.clone(),
                    preview.preview_token.as_deref().unwrap(),
                    database.clone(),
                )
                .unwrap()
                .1
            });
            let package = fixture.package();
            assert_eq!(
                fs::read(package.join(PLUGIN)).unwrap(),
                b"unsupported dll A"
            );
            fs::write(package.join(PLUGIN), b"unsupported dll B").unwrap();
            let changed = fixture
                .state
                .plugin_selection
                .inventory(&scope)
                .unwrap()
                .unwrap();
            assert!(!changed.candidates[0].selected);
            assert_ne!(
                original.selection.inventory_id(),
                changed.selection.inventory_id()
            );
            let game = snapshot_file_tree(&fixture.game);
            let pending_root = fixture.app_data.join("install/plugin-selections");
            let pending = pending_root
                .exists()
                .then(|| snapshot_file_tree(&pending_root));
            if let Some(sealed) = sealed {
                let (_, result) = crate::BatchLifecycleAutomation::start_request_with_database(
                    &fixture.environment,
                    &sealed.batch_id,
                    &sealed.plan_token,
                    database,
                )
                .unwrap();
                assert_eq!(
                    result.status,
                    hmm_core::BatchAttemptStatus::Blocked,
                    "{result:?}"
                );
                assert_eq!(result.summary.blocked_count, 1);
            } else {
                assert!(
                    crate::BatchLifecycleAutomation::seal_request_with_database(
                        &fixture.environment,
                        request,
                        preview.preview_token.as_deref().unwrap(),
                        database,
                    )
                    .is_err(),
                    "accepted changed excluded bytes: upgrade={upgrade}"
                );
            }
            assert_eq!(snapshot_file_tree(&fixture.game), game);
            assert_eq!(
                pending_root
                    .exists()
                    .then(|| snapshot_file_tree(&pending_root)),
                pending
            );
            assert_no_reinstall_recovery_transactions(&fixture.app_data);
        }
    }
}

#[test]
fn sealed_batch_rejects_changed_plugin_choice_or_applied_snapshot_without_approval() {
    for change in ["choice", "applied"] {
        let fixture = AutomationFixture::new();
        fixture.install();
        let archive = fixture._temp.path().join("candidate.zip");
        create_fixture_zip(&archive, &[(MODEL, b"new model"), (PLUGIN, &fixture.dll)]);
        let revision = import_candidate_fixture_revision(
            &fixture.state,
            &archive,
            &fixture.mod_id,
            &fixture.scope().revision_id,
        )
        .1;
        let scope = hmm_core::PluginSelectionScope {
            revision_id: revision.clone(),
            ..fixture.scope()
        };
        let request = request(&fixture, revision, true);
        let preview =
            crate::BatchLifecycleAutomation::preview_request(&fixture.environment, request.clone())
                .unwrap();
        assert_eq!(preview.plan.status(), BatchPlanStatus::Ready, "{preview:?}");
        let database = fixture.state.database_handle();
        let (_, sealed) = crate::BatchLifecycleAutomation::seal_request_with_database(
            &fixture.environment,
            request,
            preview.preview_token.as_deref().unwrap(),
            database.clone(),
        )
        .unwrap();
        if change == "choice" {
            let inventory = fixture
                .state
                .plugin_selection
                .inventory(&scope)
                .unwrap()
                .unwrap();
            fixture
                .state
                .plugin_selection
                .select(&scope, &inventory.selection.inventory_id(), &[])
                .unwrap();
        } else {
            let mut manifest = read_fixture_manifest(&fixture.app_data);
            let old = &manifest.plugin_selections[0];
            manifest.plugin_selections[0] = hmm_core::PluginSelectionSnapshot::new(
                old.scope().clone(),
                old.policy_id(),
                old.policy_version() + 1,
                old.files().to_vec(),
            )
            .unwrap();
            fixture.manifests.save_manifest(&manifest).unwrap();
        }
        let game = snapshot_file_tree(&fixture.game);
        let manifest = read_fixture_manifest(&fixture.app_data);
        let pending_root = fixture.app_data.join("install/plugin-selections");
        let pending = snapshot_file_tree(&pending_root);
        let (_, result) = crate::BatchLifecycleAutomation::start_request_with_database(
            &fixture.environment,
            &sealed.batch_id,
            &sealed.plan_token,
            database,
        )
        .unwrap();
        assert_eq!(
            result.status,
            hmm_core::BatchAttemptStatus::Blocked,
            "accepted changed {change}: {result:?}"
        );
        assert_eq!(result.summary.blocked_count, 1);
        assert_eq!(snapshot_file_tree(&fixture.game), game);
        assert_eq!(read_fixture_manifest(&fixture.app_data), manifest);
        assert_eq!(snapshot_file_tree(&pending_root), pending);
        assert_no_reinstall_recovery_transactions(&fixture.app_data);
    }
}
