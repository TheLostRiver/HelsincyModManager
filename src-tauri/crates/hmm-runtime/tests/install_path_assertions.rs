//! 安装 / 卸载的真实文件系统路径断言。
//!
//! 与 hmm-app 里基于内存 fake 的 install 测试互补：那些测试断言"服务调用了写入
//! 接口、参数是这个逻辑路径"，无法证明真实文件系统上文件落在哪里。路径拼接、
//! Windows 分隔符与大小写、父目录创建这些只有跑真实 std::fs 才暴露。
//!
//! 这里的断言方式是**整目录快照对比**而不是逐个 assert 单文件：
//! 逐个断言只能证明"我想到要检查的那些文件对了"，证明不了"没有多写别的文件"。
//! 误写到游戏目录其他位置、卸载残留、临时文件未清理都属于后者，
//! 而这类问题恰恰是玩家数据安全的核心风险。
//!
//! 有了这层断言，验收时不需要人工逐个核对路径。

use hmm_app::{
    CommitInstallPlanRequest, InstallCommitService, UninstallModRequest, UninstallModService,
};
use hmm_core::{
    FileLayer, InstallFileProvider, InstallPlan, InstallTargetPath, ModId, PackageFileId, ProfileId,
};
use hmm_infra::{
    FileSystemInstallBackupStore, FileSystemInstallGameFileSystem,
    FileSystemInstallSourceFileReader, JsonInstallManifestRepository,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

/// 游戏目录的完整内容快照：相对路径 -> 内容 sha256。
///
/// 用 hash 而不是原始字节：断言失败时输出不会被二进制内容淹没，
/// 且 BTreeMap 的有序性让 assert_eq! 的 diff 稳定可读。
fn snapshot_tree(root: &Path) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();
    collect_tree(root, root, &mut entries);
    entries
}

fn collect_tree(root: &Path, current: &Path, entries: &mut BTreeMap<String, String>) {
    let read_dir = match fs::read_dir(current) {
        Ok(read_dir) => read_dir,
        Err(_) => return,
    };

    for entry in read_dir.filter_map(Result::ok) {
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };

        if metadata.is_dir() {
            collect_tree(root, &path, entries);
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .expect("collected path must live under the snapshot root")
            .to_string_lossy()
            // 统一分隔符：Windows 下 std::fs 给出 `\`，断言里用 `/` 才能跨平台比较。
            .replace('\\', "/");
        let bytes = fs::read(&path).unwrap_or_default();
        let digest = Sha256::digest(&bytes);
        entries.insert(relative, format!("{digest:x}"));
    }
}

fn digest_of(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct InstallFixture {
    _temp: tempfile::TempDir,
    game_root: std::path::PathBuf,
    commit: InstallCommitService,
    uninstall: UninstallModService,
}

fn fixture() -> InstallFixture {
    let temp = tempfile::tempdir().expect("temp dir");
    let source_root = temp.path().join("source-package");
    let game_root = temp.path().join("game");
    let backup_root = temp.path().join("backups");
    let manifest_root = temp.path().join("manifests");
    fs::create_dir_all(&game_root).expect("create game root");
    fs::create_dir_all(&source_root).expect("create source root");

    let game_files: Arc<_> = Arc::new(FileSystemInstallGameFileSystem::new(game_root.clone()));
    let backups: Arc<_> = Arc::new(FileSystemInstallBackupStore::new(backup_root));
    let manifests: Arc<_> = Arc::new(JsonInstallManifestRepository::new(manifest_root));

    InstallFixture {
        commit: InstallCommitService::new(
            Arc::new(FileSystemInstallSourceFileReader::new(source_root)),
            game_files.clone(),
            backups.clone(),
            manifests.clone(),
        ),
        uninstall: UninstallModService::new(game_files, backups, manifests),
        game_root,
        _temp: temp,
    }
}

fn write_source(fixture: &InstallFixture, relative: &str, bytes: &[u8]) {
    let path = fixture
        ._temp
        .path()
        .join("source-package")
        .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().expect("source file has a parent"))
        .expect("create source parent");
    fs::write(path, bytes).expect("write source file");
}

fn plan_for(mod_id: &str, targets: &[&str]) -> InstallPlan {
    InstallPlan::from_providers(
        targets
            .iter()
            .map(|target| {
                InstallFileProvider::new(
                    ModId::new(mod_id),
                    PackageFileId::new(*target),
                    InstallTargetPath::parse(*target, ["nativePC"]).expect("valid target"),
                    FileLayer::new("base", 0),
                )
            })
            .collect::<Vec<_>>(),
    )
}

#[test]
fn install_writes_exactly_the_declared_target_paths_on_a_real_filesystem() {
    let fixture = fixture();
    let targets = [
        "nativePC/models/player.mod3",
        "nativePC/models/nested/deep/armor.mod3",
        "nativePC/textures/skin.tex",
    ];
    for (index, target) in targets.iter().enumerate() {
        write_source(&fixture, target, format!("payload-{index}").as_bytes());
    }

    fixture
        .commit
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan: plan_for("mod-a", &targets),
        })
        .expect("commit should succeed");

    // 整目录对比：证明声明的路径都落对了，且没有多写任何文件。
    let expected = targets
        .iter()
        .enumerate()
        .map(|(index, target)| {
            (
                (*target).to_owned(),
                digest_of(format!("payload-{index}").as_bytes()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(snapshot_tree(&fixture.game_root), expected);
}

#[test]
fn uninstall_restores_the_game_directory_to_its_exact_initial_state() {
    let fixture = fixture();
    // 预置一个"玩家原有文件"，它不属于任何 Mod，卸载后必须原样保留。
    let untouched = fixture.game_root.join("nativePC/vanilla/original.pak");
    fs::create_dir_all(untouched.parent().expect("parent")).expect("create vanilla dir");
    fs::write(&untouched, b"vanilla content").expect("write vanilla file");
    // 预置一个会被 Mod 覆盖的文件，卸载后必须从备份精确复原。
    let overwritten = fixture.game_root.join("nativePC/models/player.mod3");
    fs::create_dir_all(overwritten.parent().expect("parent")).expect("create models dir");
    fs::write(&overwritten, b"original model").expect("write original model");

    let initial = snapshot_tree(&fixture.game_root);

    let targets = ["nativePC/models/player.mod3", "nativePC/models/new.mod3"];
    write_source(&fixture, targets[0], b"modded model");
    write_source(&fixture, targets[1], b"brand new file");
    fixture
        .commit
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan: plan_for("mod-a", &targets),
        })
        .expect("commit should succeed");
    assert_ne!(
        snapshot_tree(&fixture.game_root),
        initial,
        "install must actually change the game directory"
    );

    fixture
        .uninstall
        .uninstall_mod(UninstallModRequest {
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
        })
        .expect("uninstall should succeed");

    // 逐文件哈希对比回到初始态：覆盖的文件已复原，新增的文件已移除，
    // 玩家原有文件未被动过。任何残留或误删都会让这个断言失败。
    assert_eq!(snapshot_tree(&fixture.game_root), initial);
}

#[test]
fn install_leaves_no_temporary_artifacts_in_the_game_directory() {
    let fixture = fixture();
    let target = "nativePC/models/player.mod3";
    write_source(&fixture, target, b"payload");

    fixture
        .commit
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan: plan_for("mod-a", &[target]),
        })
        .expect("commit should succeed");

    // 原子写会在目标目录建临时文件再 rename。留下 .tmp 说明清理有漏，
    // 玩家会在游戏目录里看到垃圾文件，也可能被游戏本体误读。
    //
    // 先断言目标文件确实存在：否则一旦快照因为别的原因收集不到任何文件，
    // "没有 .tmp"会在空集合上平凡成立，这个测试就变成了摆设。
    let snapshot = snapshot_tree(&fixture.game_root);
    assert_eq!(
        snapshot.get(target),
        Some(&digest_of(b"payload")),
        "installed target must be present before asserting on leftovers"
    );
    let leftovers = snapshot
        .into_keys()
        .filter(|path| path.ends_with(".tmp"))
        .collect::<Vec<_>>();
    assert_eq!(leftovers, Vec::<String>::new());
}

#[test]
fn install_target_paths_are_not_flattened_or_case_folded() {
    let fixture = fixture();
    // 同名文件分处不同层级，且大小写不同：路径拼接一旦被压平或折叠大小写，
    // 这两个文件会互相覆盖，只剩一个。
    let targets = [
        "nativePC/models/Player.mod3",
        "nativePC/models/sub/Player.mod3",
    ];
    write_source(&fixture, targets[0], b"top level");
    write_source(&fixture, targets[1], b"nested");

    fixture
        .commit
        .commit_plan(CommitInstallPlanRequest {
            profile_id: ProfileId::new("default"),
            plan: plan_for("mod-a", &targets),
        })
        .expect("commit should succeed");

    let snapshot = snapshot_tree(&fixture.game_root);
    assert_eq!(snapshot.len(), 2, "nested same-name files must not collide");
    assert_eq!(
        snapshot.get("nativePC/models/Player.mod3"),
        Some(&digest_of(b"top level"))
    );
    assert_eq!(
        snapshot.get("nativePC/models/sub/Player.mod3"),
        Some(&digest_of(b"nested"))
    );
}
