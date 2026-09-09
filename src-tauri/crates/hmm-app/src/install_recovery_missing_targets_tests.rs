use super::*;
use hmm_core::GameId;
use hmm_ports::{GameRunningDetector, GameRunningStatus};

struct RunningState(Mutex<GameRunningStatus>);

impl GameRunningDetector for RunningState {
    fn game_running_status(&self, _game_id: &GameId) -> GameRunningStatus {
        *self.0.lock().unwrap()
    }
}

struct Fixture {
    files: Arc<FakeGameFiles>,
    records: Arc<FakeRecoveryRecords>,
    transactions: Arc<FakeReinstallTransactions>,
    running: Arc<RunningState>,
    previewer: InstallRecoveryActionPreviewService,
    executor: InstallRecoveryActionService,
}

impl Fixture {
    fn new() -> Self {
        let files = Arc::new(FakeGameFiles::default());
        let backups = Arc::new(FakeBackups::default());
        let manifests = Arc::new(FakeManifests {
            manifest: Some(InstallManifest::completed(
                ProfileId::new("default"),
                vec![InstallManifestEntry {
                    target_path: InstallTargetPath::parse("nativePC/missing.bin", ["nativePC"])
                        .unwrap(),
                    mod_id: ModId::new("mod-a"),
                    revision_id: None,
                    package_file_id: PackageFileId::new("nativePC/missing.bin"),
                    layer: FileLayer::new("base", 0),
                    backup_ref: None,
                    installed_file: Some(summary(b"installed fixture")),
                    adopted: false,
                }],
            )),
        });
        let records = Arc::new(FakeRecoveryRecords::default());
        let transactions = Arc::new(FakeReinstallTransactions::default());
        let running = Arc::new(RunningState(Mutex::new(GameRunningStatus::NotRunning)));
        let uninstaller = UninstallModService::new(files.clone(), backups.clone(), manifests)
            .with_game_running_detector(running.clone());
        let previewer = InstallRecoveryActionPreviewService::new(
            files.clone(),
            backups.clone(),
            records.clone(),
        )
        .with_missing_target_uninstall(
            GameId::mhw(),
            uninstaller.clone(),
            transactions.clone(),
        );
        let executor = InstallRecoveryActionService::new(files.clone(), backups, records.clone())
            .with_missing_target_uninstall(GameId::mhw(), uninstaller, transactions.clone());
        Self {
            files,
            records,
            transactions,
            running,
            previewer,
            executor,
        }
    }

    fn preview(&self) -> InstallRecoveryActionPreview {
        self.previewer
            .preview(InstallRecoveryActionPreviewRequest {
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                action_kind: InstallRecoveryActionKind::UninstallMissingTargets,
            })
            .unwrap()
    }

    fn run(
        &self,
        token: Option<String>,
    ) -> Result<InstallRecoveryActionResult, InstallRecoveryActionError> {
        self.executor.run(InstallRecoveryActionRequest {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            action_kind: InstallRecoveryActionKind::UninstallMissingTargets,
            plan_token: token,
        })
    }

    fn assert_untouched(&self) {
        assert!(self.files.writes.lock().unwrap().is_empty());
        assert!(self.files.removals.lock().unwrap().is_empty());
        assert!(self.records.removed_records.lock().unwrap().is_empty());
        assert_eq!(*self.transactions.remove_count.lock().unwrap(), 0);
        assert_eq!(*self.transactions.save_count.lock().unwrap(), 0);
        // FakeManifests 和 FakeBackups 对任何持久化操作都会 panic。
    }

    fn assert_blocked(&self, token: Option<String>, reason: InstallRecoveryActionBlockReason) {
        let expected = vec![InstallRecoveryActionBlockReasonSummary { reason, count: 1 }];
        let preview = self.preview();
        assert_eq!(
            preview.availability,
            InstallRecoveryActionAvailability::Blocked
        );
        assert_eq!(preview.blocking_reasons, expected);
        assert!(preview.plan_token.is_none());
        assert_eq!(
            self.run(token),
            Err(InstallRecoveryActionError::Blocked { reasons: expected })
        );
        self.assert_untouched();
    }
}

#[test]
fn missing_target_action_requires_a_canonical_preview_token() {
    let fixture = Fixture::new();
    let preview = fixture.preview();
    assert_eq!(
        preview.availability,
        InstallRecoveryActionAvailability::Available
    );
    assert_eq!(preview.missing_file_count, 1);
    for token in [
        None,
        Some(String::new()),
        Some("missing-uninstall-v1:abc".to_owned()),
        Some(format!("missing-uninstall-v1:{}", "A".repeat(64))),
        Some(format!(" {}", preview.plan_token.unwrap())),
    ] {
        assert_eq!(
            fixture.run(token),
            Err(InstallRecoveryActionError::Blocked {
                reasons: vec![InstallRecoveryActionBlockReasonSummary {
                    reason: InstallRecoveryActionBlockReason::PreviewRequired,
                    count: 1,
                }],
            })
        );
    }
    fixture.assert_untouched();
}

#[test]
fn missing_target_action_checks_game_state_again_after_preview() {
    for (status, reason, error) in [
        (
            GameRunningStatus::Running,
            InstallRecoveryActionBlockReason::GameRunning,
            UninstallModError::GameRunning,
        ),
        (
            GameRunningStatus::Unknown,
            InstallRecoveryActionBlockReason::GameRunningUnknown,
            UninstallModError::GameRunningUnknown,
        ),
    ] {
        let fixture = Fixture::new();
        let token = fixture.preview().plan_token;
        assert!(token.is_some());
        *fixture.running.0.lock().unwrap() = status;
        let blocked = fixture.preview();
        assert_eq!(
            blocked.availability,
            InstallRecoveryActionAvailability::Blocked
        );
        assert_eq!(
            blocked.blocking_reasons,
            vec![InstallRecoveryActionBlockReasonSummary { reason, count: 1 }]
        );
        assert!(blocked.plan_token.is_none());
        assert_eq!(
            fixture.run(token),
            Err(InstallRecoveryActionError::MissingTargetUninstall(error))
        );
        fixture.assert_untouched();
    }
}

#[test]
fn missing_target_action_rechecks_pending_install_records_across_the_profile() {
    for mod_id in ["mod-a", "other-mod"] {
        for status in [
            InstallRecoveryRecordStatus::Planned,
            InstallRecoveryRecordStatus::Committing,
            InstallRecoveryRecordStatus::RollbackRequired,
            InstallRecoveryRecordStatus::RepairRequired,
        ] {
            let fixture = Fixture::new();
            let token = fixture.preview().plan_token;
            assert!(token.is_some());
            fixture.records.insert(recovery_record(
                status,
                InstallTargetPath::parse("nativePC/pending.bin", ["nativePC"]).unwrap(),
                ModId::new(mod_id),
                Some(summary(b"pending fixture")),
                None,
            ));
            fixture.assert_blocked(token, InstallRecoveryActionBlockReason::RecoveryPending);
        }
    }
}

#[test]
fn missing_target_preview_allows_terminal_install_records_and_ignores_other_profiles() {
    for status in [
        InstallRecoveryRecordStatus::Completed,
        InstallRecoveryRecordStatus::RolledBack,
        InstallRecoveryRecordStatus::Committing,
    ] {
        let fixture = Fixture::new();
        let mut record = recovery_record(
            status,
            InstallTargetPath::parse("nativePC/old.bin", ["nativePC"]).unwrap(),
            ModId::new("other-mod"),
            Some(summary(b"fixture")),
            None,
        );
        if status == InstallRecoveryRecordStatus::Committing {
            record.profile_id = ProfileId::new("other-profile");
        }
        fixture.records.insert(record);
        assert_eq!(
            fixture.preview().availability,
            InstallRecoveryActionAvailability::Available
        );
        fixture.assert_untouched();
    }
}

#[test]
fn missing_target_action_preserves_every_pending_reinstall_transaction() {
    for status in [
        ReinstallRecoveryTransactionStatus::Planned,
        ReinstallRecoveryTransactionStatus::Committing,
        ReinstallRecoveryTransactionStatus::Completed,
        ReinstallRecoveryTransactionStatus::RollbackRequired,
        ReinstallRecoveryTransactionStatus::RolledBack,
        ReinstallRecoveryTransactionStatus::RepairRequired,
    ] {
        let fixture = Fixture::new();
        let token = fixture.preview().plan_token;
        assert!(token.is_some());
        let (_, _, mut transaction, _) = reinstall_recovery_fixture();
        transaction.status = status;
        fixture.transactions.insert(transaction);
        fixture.assert_blocked(token, InstallRecoveryActionBlockReason::RecoveryPending);
    }
}

#[test]
fn missing_target_action_fails_closed_when_recovery_evidence_cannot_be_listed() {
    for fail_installs in [false, true] {
        let fixture = Fixture::new();
        let token = fixture.preview().plan_token;
        assert!(token.is_some());
        if fail_installs {
            *fixture.records.fail_lists.lock().unwrap() = true;
        } else {
            *fixture.transactions.fail_lists.lock().unwrap() = true;
        }
        fixture.assert_blocked(
            token,
            InstallRecoveryActionBlockReason::InstallStateUnavailable,
        );
    }
}
