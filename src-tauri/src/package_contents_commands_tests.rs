use super::*;
use hmm_app::PackageContentEntry;

fn entry(package_file_id: &str, target_path: Option<&str>) -> PackageContentEntry {
    PackageContentEntry {
        package_file_id: package_file_id.to_owned(),
        size_bytes: 42,
        target_path: target_path.map(str::to_owned),
        installable: target_path.is_some(),
        rejected_by_game: false,
    }
}

#[test]
fn a_single_content_root_carries_its_path_and_no_candidates() {
    let dto = PackageContentRootDto::from(PackageContentRoot::Single("黑骑士大剑".to_owned()));

    assert_eq!(dto.kind, "single");
    assert_eq!(dto.path.as_deref(), Some("黑骑士大剑"));
    assert!(dto.candidates.is_empty());
}

/// 回退到沙箱根时 `path` 是**空串**而不是 `None`：根定下来了，只是它就是沙箱根本身。
/// 与 `ambiguous` 的 `None`（根还没定）必须能区分，否则 UI 分不清「装到根」和「等你挑」。
#[test]
fn a_fallback_content_root_is_an_empty_path_not_a_missing_one() {
    let dto = PackageContentRootDto::from(PackageContentRoot::Fallback);

    assert_eq!(dto.kind, "fallback");
    assert_eq!(dto.path.as_deref(), Some(""));
    assert!(dto.candidates.is_empty());
}

/// 多个 `nativePC`：候选如实带出、`path` 为 `None`。这是 D2 让玩家挑的输入，
/// 也是这条命令与 `preview_imported_mod_install_plan` 的关键差别——那边此时直接报错。
#[test]
fn an_ambiguous_content_root_exposes_the_candidates_for_the_player_to_choose() {
    let dto = PackageContentRootDto::from(PackageContentRoot::Ambiguous(vec![
        "大剑".to_owned(),
        "太刀".to_owned(),
    ]));

    assert_eq!(dto.kind, "ambiguous");
    assert_eq!(dto.path, None);
    assert_eq!(dto.candidates, vec!["大剑".to_owned(), "太刀".to_owned()]);
}

/*
 * DTO **不得**把 `installable` 与 `rejected_by_game` 合并。
 *
 * 拒绝清单当前只在重定向链路上被强制执行，普通安装链路尚未套用，所以同一个 `.exe`
 * 两条链路的归宿不同。命令层若合并成一个布尔，就必然在其中一条链路上给出与实际相反的
 * 答案——这条断言钉住「两个字段同时为真」这个状态能原样穿到前端。
 */
#[test]
fn the_dto_keeps_installability_and_reject_list_hits_as_separate_fields() {
    let dto = PackageContentsDto::from(PackageContents {
        candidates: vec![String::new()],
        content_root: PackageContentRoot::Fallback,
        entries: vec![PackageContentEntry {
            package_file_id: "nativePC/wp/two/bs_two012/mod/MHWTexConverter_by_Jodo.exe".to_owned(),
            size_bytes: 30208,
            target_path: Some(
                "nativePC/wp/two/bs_two012/mod/MHWTexConverter_by_Jodo.exe".to_owned(),
            ),
            installable: true,
            rejected_by_game: true,
        }],
    });

    let entry = &dto.entries[0];
    assert!(entry.installable);
    assert!(entry.rejected_by_game);
}

/// 不在内容根之下的文件照常出现在清单里，只是没有目标路径——玩家要看得见整包。
#[test]
fn entries_outside_the_content_root_survive_the_projection() {
    let dto = PackageContentsDto::from(PackageContents {
        candidates: vec![String::new(), "黑骑士大剑".to_owned()],
        content_root: PackageContentRoot::Single("黑骑士大剑".to_owned()),
        entries: vec![
            entry("readme.txt", None),
            entry(
                "黑骑士大剑/nativePC/wp/two003.mod3",
                Some("nativePC/wp/two003.mod3"),
            ),
        ],
    });

    assert_eq!(dto.entries.len(), 2);
    assert_eq!(dto.entries[0].target_path, None);
    assert!(!dto.entries[0].installable);
    assert_eq!(
        dto.entries[1].target_path.as_deref(),
        Some("nativePC/wp/two003.mod3")
    );
}

#[test]
fn a_blank_mod_id_is_rejected_before_any_scan() {
    let error = package_contents_request_from_dto(PackageContentsRequestDto {
        game_id: Some("mhw".to_owned()),
        mod_id: Some("   ".to_owned()),
    })
    .expect_err("空 Mod id 必须在扫描之前就被挡下");

    assert_eq!(error.code, "package_contents_mod_id_invalid");
}

#[test]
fn an_unknown_game_id_is_rejected_before_any_scan() {
    let error = package_contents_request_from_dto(PackageContentsRequestDto {
        game_id: Some("!!!".to_owned()),
        mod_id: Some("mod-a".to_owned()),
    })
    .expect_err("非法 game id 必须在扫描之前就被挡下");

    assert_eq!(error.code, "package_contents_game_id_invalid");
}

/// 扫描侧的稳定错误码原样透出，命令层不重新发明——诊断串起来才追得下去。
#[test]
fn scan_error_codes_reach_the_command_layer_unchanged() {
    let error = package_contents_error_to_command_error(PackageContentsQueryError::ScanFailed(
        hmm_ports::ModPackageInstallFileScanError::DepthLimitExceeded,
    ));

    assert_eq!(error.code, "imported_mod_file_scan_depth_limit_exceeded");
}
