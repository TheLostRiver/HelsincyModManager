//! 生产装配必须给安装与卸载接上游戏运行中闸门。
//!
//! `InstallCommitService` / `UninstallModService` 的 detector 字段是 `Option`，
//! 缺席时放行——这让大量只用 fake 文件系统的单元测试不必各自装配 detector，
//! 代价是类型系统挡不住"生产装配忘了接线"。
//!
//! 这个 tripwire 就是补上那一层：断言两处生产装配点确实调用了
//! `with_game_running_detector`。谁把接线删掉，这里先红。
//!
//! 它只能证明"接线还在"，证明不了闸门语义正确——语义由
//! `hmm-app` 的 `install_game_running_tests.rs` 覆盖（拒绝、且无任何副作用）。

const COMPOSITION: &str = include_str!("../src/composition.rs");
const UNINSTALL: &str = include_str!("../src/uninstall.rs");

#[test]
fn install_commit_wiring_attaches_the_game_running_detector() {
    assert!(
        COMPOSITION.contains(".with_game_running_detector("),
        "composition.rs 构造 InstallCommitService 时必须接上游戏运行中闸门"
    );
}

#[test]
fn uninstall_wiring_attaches_the_game_running_detector() {
    assert!(
        UNINSTALL.contains(".with_game_running_detector("),
        "uninstall.rs 构造 UninstallModService 时必须接上游戏运行中闸门"
    );
}

#[test]
fn install_and_uninstall_share_one_detector_instance() {
    // 安装与卸载走同一个探测器，避免两条链对"游戏是否在跑"给出不同答案。
    assert!(
        COMPOSITION.contains("install_game_running_detector"),
        "composition.rs 应复用同一个 detector 实例给安装与卸载两条链"
    );
}
