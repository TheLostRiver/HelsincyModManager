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

/// 取出以 `marker` 开头、到匹配右括号为止的构造调用片段。
///
/// 只搜字符串是否"出现过"太松：无关服务调用同名方法也能让断言通过。
/// 限定到具体构造调用内部，才能证明接线挂在**该**服务上。
fn construction_call<'a>(source: &'a str, marker: &str) -> &'a str {
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("未找到构造调用: {marker}"));
    let rest = &source[start..];
    let mut depth = 0usize;
    for (offset, ch) in rest.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    // 带上紧随其后的链式调用，直到该语句结束。
                    let tail = &rest[offset..];
                    let end = tail
                        .find(
                            ";
",
                        )
                        .map_or(tail.len(), |at| at + 1);
                    return &rest[..offset + end];
                }
            }
            _ => {}
        }
    }
    panic!("构造调用括号未闭合: {marker}");
}

#[test]
fn install_commit_wiring_attaches_the_shared_game_running_detector() {
    let call = construction_call(COMPOSITION, "ConfiguredInstallCommitter::new(");
    assert!(
        call.contains("Arc::clone(&install_game_running_detector)"),
        "ConfiguredInstallCommitter::new 必须接上共享的游戏运行中探测器"
    );
}

#[test]
fn uninstall_wiring_attaches_the_shared_game_running_detector() {
    let call = construction_call(COMPOSITION, "crate::uninstall::mod_uninstaller(");
    assert!(
        call.contains("Arc::clone(&install_game_running_detector)"),
        "mod_uninstaller 必须接上共享的游戏运行中探测器"
    );
    assert!(
        UNINSTALL.contains(".with_game_running_detector("),
        "uninstall.rs 构造 UninstallModService 时必须把探测器接进服务"
    );
}

#[test]
fn install_and_uninstall_share_one_detector_instance() {
    // 两条链必须共用同一个实例，否则可能对"游戏是否在跑"给出不同答案。
    // 只检查标识符出现过是不够的：两个独立 detector 也会让那种断言通过。
    let install = construction_call(COMPOSITION, "ConfiguredInstallCommitter::new(");
    let uninstall = construction_call(COMPOSITION, "crate::uninstall::mod_uninstaller(");
    let shared = "Arc::clone(&install_game_running_detector)";
    assert!(
        install.contains(shared) && uninstall.contains(shared),
        "安装与卸载必须复用同一个 detector 实例"
    );
    assert_eq!(
        COMPOSITION
            .matches("let install_game_running_detector =")
            .count(),
        1,
        "共享实例只应被创建一次"
    );
}
