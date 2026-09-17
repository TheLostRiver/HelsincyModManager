use super::*;
use hmm_core::{
    BatchExecutionPolicy, GameId, InstallBatchItemInput, ModRevisionId, ProfileId,
    UninstallBatchItemInput, BATCH_PLAN_SCHEMA_VERSION,
};

fn fixture(
    count: usize,
) -> (
    tempfile::TempDir,
    RuntimeEnvironment,
    BatchLifecyclePlanRequest,
) {
    let root = tempfile::tempdir().unwrap();
    crate::lifecycle_automation::write_install_fixture(root.path());
    let mut records = Vec::new();
    let mut items = Vec::new();
    for i in 0..count {
        let mod_id = format!("mod-{i}");
        let revision = format!("package-{i}");
        records.push(serde_json::json!({ "mod_id": mod_id, "task_id": format!("task-{i}"), "package_id": revision, "display_name": mod_id }));
        let directory = root
            .path()
            .join("mod-import/sandboxes")
            .join(&revision)
            .join("nativePC/models");
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join(format!("fixture-{i}.bin")),
            b"artificial-mod",
        )
        .unwrap();
        items.push(BatchItemInput::Install(InstallBatchItemInput {
            mod_id: ModId::new(mod_id),
            revision_id: ModRevisionId::new(revision),
            layer: hmm_core::FileLayer::new("base", 0),
            replacement_binding_snapshot: None,
        }));
    }
    fs::write(
        root.path().join("mod-import/results.json"),
        serde_json::to_vec(&serde_json::json!({ "version": 1, "records": records })).unwrap(),
    )
    .unwrap();
    let environment = RuntimeEnvironment::sandbox(root.path().to_path_buf()).unwrap();
    let request = BatchPlanRequest {
        schema_version: BATCH_PLAN_SCHEMA_VERSION,
        operation: BatchOperation::Install,
        game_id: GameId::mhw(),
        profile_id: ProfileId::new("default"),
        execution_policy: BatchExecutionPolicy::StopOnFailure,
        items,
    };
    (root, environment, request.into())
}

#[test]
fn per_item_install_facts_match_full_plan_with_a_stable_environment_identity() {
    for count in [5, 10, 20] {
        let (_root, environment, request) = fixture(count);
        let request = request.plan.normalize().unwrap();
        let provider = BatchFactsProvider { environment };
        let full = provider.read_batch_plan_facts(&request).unwrap();
        for item in &request.items {
            let current = provider
                .read_batch_item_facts(&request, item.mod_id())
                .unwrap();
            assert_eq!(current.environment_digest, full.environment_digest);
            assert_eq!(
                current.prerequisite_rules_version,
                full.prerequisite_rules_version
            );
            assert_eq!(current.items.len(), 1);
            assert_eq!(
                &current.items[0],
                full.items
                    .iter()
                    .find(|fact| &fact.mod_id == item.mod_id())
                    .unwrap()
            );
        }
        let mut changed_other = request.clone();
        let BatchItemInput::Install(other) = &mut changed_other.items[1] else {
            unreachable!()
        };
        other.revision_id = ModRevisionId::new("missing-revision");
        assert!(provider.read_batch_plan_facts(&changed_other).is_err());
        // Revalidation of the first item must not rebuild another item's missing source.
        assert!(provider
            .read_batch_item_facts(&changed_other, request.items[0].mod_id())
            .is_ok());
        assert!(provider
            .read_batch_item_facts(&changed_other, request.items[1].mod_id())
            .is_err());
    }
}

#[test]
fn batch_install_then_uninstall_commits_each_item_through_existing_transactions() {
    let (root, environment, install) = fixture(10);
    let uninstall = BatchPlanRequest {
        operation: BatchOperation::Uninstall,
        items: install
            .plan
            .items
            .iter()
            .map(|item| {
                let BatchItemInput::Install(item) = item else {
                    unreachable!()
                };
                BatchItemInput::Uninstall(UninstallBatchItemInput {
                    mod_id: item.mod_id.clone(),
                    expected_installed_revision_id: item.revision_id.clone(),
                })
            })
            .collect(),
        ..install.plan.clone()
    }
    .into();
    let sentinel = root
        .path()
        .join("fixtures/games/mhw-minimal/nativePC/unowned.bin");
    fs::write(&sentinel, b"unowned-fixture").unwrap();
    for request in [install, uninstall] {
        let preview =
            BatchLifecycleAutomation::preview_request(&environment, request.clone()).unwrap();
        let (_, sealed) = BatchLifecycleAutomation::seal_request(
            &environment,
            request,
            &preview.preview_token.unwrap(),
        )
        .unwrap();
        let (_, run) = BatchLifecycleAutomation::start_request(
            &environment,
            &sealed.batch_id,
            &sealed.plan_token,
        )
        .unwrap();
        assert_eq!(run.status, BatchAttemptStatus::Completed);
        assert_eq!(run.summary.succeeded_count, 10);
        let snapshot = BatchLifecycleAutomation::result(&environment, &sealed.batch_id, 0).unwrap();
        assert_eq!(snapshot.summary.succeeded_count, 10);
    }
    assert_eq!(fs::read(sentinel).unwrap(), b"unowned-fixture");
    for i in 0..10 {
        assert!(!root
            .path()
            .join(format!(
                "fixtures/games/mhw-minimal/nativePC/models/fixture-{i}.bin"
            ))
            .exists());
    }
}
