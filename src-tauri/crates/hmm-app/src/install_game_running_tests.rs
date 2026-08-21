//! 游戏运行中不得写入玩家文件。
//!
//! 这条闸门在 Windows Sandbox 里结构性地测不到——那里永远没有游戏在跑。
//! 真机上 MHW:I 会持有 `nativePC` 下的文件句柄：写入触发 sharing violation，
//! 随后的 rollback 要写回同一批仍被锁的文件、同样会失败，把一次普通的安装失败
//! 升级成需要人工恢复的 `RollbackRequired`。
//!
//! 因此这里除了断言"被拒绝"，更重要的是断言 **拒绝时没有任何副作用**：
//! 没有写游戏文件、没有建 backup、没有落 recovery 记录、没有存 manifest。

use super::*;
use hmm_core::{FileLayer, GameId, ModId, PackageFileId, ProfileId};
use hmm_ports::{GameRunningDetector, GameRunningStatus};
use std::sync::Arc;

struct StubGameRunningDetector(GameRunningStatus);

impl GameRunningDetector for StubGameRunningDetector {
    fn game_running_status(&self, _game_id: &GameId) -> GameRunningStatus {
        self.0
    }
}

fn single_file_plan() -> InstallPlan {
    InstallPlanningService::new()
        .build_plan(BuildInstallPlanRequest {
            allowed_target_roots: vec!["nativePC".to_owned()],
            files: vec![InstallPlanFile {
                mod_id: ModId::new("mod-a"),
                package_file_id: PackageFileId::new("file-a"),
                target_path: "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3".to_owned(),
                layer: FileLayer::new("test", 0),
            }],
        })
        .expect("plan should build")
}

struct CommitFixture {
    service: InstallCommitService,
    game_files: Arc<RecordingInstallGameFileSystem>,
    backups: Arc<RecordingInstallBackupStore>,
    manifests: Arc<RecordingInstallManifestRepository>,
    recovery: Arc<RecordingInstallRecoveryRecordRepository>,
}

fn commit_fixture(status: GameRunningStatus) -> CommitFixture {
    // reader 按 package_file_id 取内容，不是按目标路径。
    let source_files = Arc::new(RecordingInstallSourceFileReader::new([(
        "file-a",
        b"retargeted armor".as_slice(),
    )]));
    // 目标位置已有官方文件：真机上覆盖前必须先备份，这样"零 backup"才是有力断言。
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
        b"vanilla armor".as_slice(),
    )]));
    let backups = Arc::new(RecordingInstallBackupStore::default());
    let manifests = Arc::new(RecordingInstallManifestRepository::default());
    let recovery = Arc::new(RecordingInstallRecoveryRecordRepository::default());

    let service = InstallCommitService::new_with_recovery_records(
        source_files,
        game_files.clone(),
        backups.clone(),
        manifests.clone(),
        recovery.clone(),
    )
    .with_game_running_detector(Arc::new(StubGameRunningDetector(status)));

    CommitFixture {
        service,
        game_files,
        backups,
        manifests,
        recovery,
    }
}

fn assert_nothing_touched(fixture: &CommitFixture) {
    assert!(
        fixture.game_files.write_requests().is_empty(),
        "闸门拒绝后不得写入任何游戏文件"
    );
    assert!(
        fixture.recovery.saved_records().is_empty(),
        "闸门拒绝后不得留下 recovery 记录，否则会被误判成待恢复状态"
    );
    assert!(
        fixture.manifests.take_manifest().is_none(),
        "闸门拒绝后不得保存 manifest"
    );
    // 这条是四项里最容易漏的：目标位置本来就有官方文件，正常路径必然会先备份。
    // 少了它，闸门被挪到 backup 之后仍能全绿，却已经留下持久化副作用。
    assert!(
        fixture.backups.store_attempts().is_empty(),
        "闸门拒绝后不得创建 backup"
    );
}

#[test]
fn commit_is_rejected_without_side_effects_while_the_game_runs() {
    let fixture = commit_fixture(GameRunningStatus::Running);

    let error = fixture
        .service
        .commit_plan(CommitInstallPlanRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            plan: single_file_plan(),
        })
        .expect_err("game running must block the commit");

    assert_eq!(error, InstallCommitError::GameRunning);
    assert_nothing_touched(&fixture);
}

#[test]
fn commit_is_rejected_when_the_game_running_state_is_unknown() {
    // 判定不出来时不能假设游戏没开：与 save_restore 的存档恢复闸门同语义。
    let fixture = commit_fixture(GameRunningStatus::Unknown);

    let error = fixture
        .service
        .commit_plan(CommitInstallPlanRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            plan: single_file_plan(),
        })
        .expect_err("unknown game state must block the commit");

    assert_eq!(error, InstallCommitError::GameRunningUnknown);
    assert_nothing_touched(&fixture);
}

#[test]
fn commit_proceeds_when_the_game_is_not_running() {
    // 负向用例的对照组：证明闸门没有把正常安装一并拦死。
    let fixture = commit_fixture(GameRunningStatus::NotRunning);

    fixture
        .service
        .commit_plan(CommitInstallPlanRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            plan: single_file_plan(),
        })
        .expect("a closed game must not block the commit");

    assert_eq!(
        fixture.game_files.write_requests(),
        vec!["nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3".to_owned()],
        "游戏没开时应正常落盘"
    );
}

#[test]
fn uninstall_is_rejected_while_the_game_runs() {
    let game_files = Arc::new(RecordingInstallGameFileSystem::with_files([(
        "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
        b"installed armor".as_slice(),
    )]));
    // 用读取即失败的 manifest repository：闸门若被挪到 load_manifest 之后，
    // 这里会拿到 ManifestUnavailable 而不是 GameRunning。空 repo 证不到这一点。
    let service = UninstallModService::new(
        game_files.clone(),
        Arc::new(RecordingInstallBackupStore::default()),
        Arc::new(RecordingInstallManifestRepository::failing_load()),
    )
    .with_game_running_detector(Arc::new(StubGameRunningDetector(
        GameRunningStatus::Running,
    )));

    let error = service
        .uninstall_mod(UninstallModRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect_err("game running must block the uninstall");

    assert_eq!(error, UninstallModError::GameRunning);
    assert!(
        game_files.write_requests().is_empty(),
        "卸载被拒时不得删除或还原任何游戏文件"
    );
}

#[test]
fn uninstall_is_rejected_when_the_game_running_state_is_unknown() {
    let service = UninstallModService::new(
        Arc::new(RecordingInstallGameFileSystem::default()),
        Arc::new(RecordingInstallBackupStore::default()),
        Arc::new(RecordingInstallManifestRepository::default()),
    )
    .with_game_running_detector(Arc::new(StubGameRunningDetector(
        GameRunningStatus::Unknown,
    )));

    let error = service
        .uninstall_mod(UninstallModRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect_err("unknown game state must block the uninstall");

    assert_eq!(error, UninstallModError::GameRunningUnknown);
}
