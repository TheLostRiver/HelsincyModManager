//! 验证尚未安装、尚未使用 HMM 重定向的人工包，也能从真实 MHW 目录识别默认装备名称。
use hmm_app::AnalyzeImportedReplacementRequest;
use hmm_core::{GameId, ModId, ProfileId};
use hmm_games_mhw::MhwReplacementCatalog;
use hmm_ports::ReplacementCatalogProvider;
use hmm_runtime::HmmRuntime;
use std::fs;
use std::path::{Path, PathBuf};

fn fixture(files: &[&str]) -> (tempfile::TempDir, HmmRuntime, PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let imported = temporary.path().join("mod-import");
    fs::create_dir_all(&imported).unwrap();
    fs::write(imported.join("results.json"), r#"{
      "version": 1,
      "records": [{"mod_id":"mod-a","task_id":"import-a","package_id":"package-a","display_name":"Artificial package"}]
    }"#).unwrap();
    let package = imported.join("sandboxes/package-a");
    for relative in files {
        let file = package.join(relative);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, b"artificial file for read-only path analysis").unwrap();
    }
    let runtime = HmmRuntime::from_app_data_dir(temporary.path().to_path_buf()).unwrap();
    (temporary, runtime, package)
}

fn assert_no_installation(data_root: &Path, package: &Path, files: &[&str]) {
    assert!(!data_root.join("install/manifests/default.json").exists());
    for relative in files {
        assert_eq!(
            fs::read(package.join(relative)).unwrap(),
            b"artificial file for read-only path analysis"
        );
    }
}

#[test]
fn original_weapon_names_are_available_without_installation_or_hmm_retargeting() {
    let files = [
        "nativePC/wp/two/two028/mod/two028.mod3",
        "nativePC/wp/two/two028/mod/two028.mrl3",
    ];
    let (temporary, runtime, package) = fixture(&files);
    let summary = runtime
        .replacement_workflow
        .replacement_summary(
            AnalyzeImportedReplacementRequest {
                game_id: GameId::mhw(),
                mod_id: ModId::new("mod-a"),
            },
            Some(&ProfileId::new("default")),
        )
        .unwrap();
    assert_eq!(summary.sources.len(), 1);
    assert_eq!(summary.sources[0].kind, "weapon");
    assert_eq!(summary.sources[0].internal_id, "two028");
    assert_eq!(summary.sources[0].display_names["zh_cn"], "狂击巨凶");
    assert_eq!(summary.sources[0].display_names["en"], "Glacial Demon");
    assert_eq!(summary.installed_targets, Some(vec![]));
    assert_no_installation(temporary.path(), &package, &files);
}

#[test]
fn original_armor_names_come_from_the_catalog_without_a_profile_or_binding() {
    let files = ["nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3"];
    let (temporary, runtime, package) = fixture(&files);
    let summary = runtime
        .replacement_workflow
        .replacement_summary(
            AnalyzeImportedReplacementRequest {
                game_id: GameId::mhw(),
                mod_id: ModId::new("mod-a"),
            },
            None,
        )
        .unwrap();
    let catalog = MhwReplacementCatalog.replacement_catalog().unwrap();
    let target = catalog
        .targets()
        .iter()
        .find(|target| {
            target.target_type().as_str() == "armor"
                && target.internal_id() == "pl129_0000"
                && target
                    .metadata()
                    .get("path_family")
                    .and_then(serde_json::Value::as_str)
                    == Some("pl/f_equip")
        })
        .unwrap();
    assert_eq!(summary.sources.len(), 1);
    assert_eq!(summary.sources[0].kind, "armor");
    assert_eq!(summary.sources[0].internal_id, "pl129_0000");
    assert_eq!(
        summary.sources[0].display_names,
        target.display_name().clone().into()
    );
    assert!(summary.sources[0]
        .display_names
        .values()
        .all(|name| !name.is_empty()));
    assert_eq!(summary.installed_targets, None);
    assert_no_installation(temporary.path(), &package, &files);
}

#[test]
fn multi_source_packages_still_describe_defaults_when_single_target_retargeting_is_blocked() {
    let files = [
        "nativePC/wp/two/two028/mod/two028.mod3",
        "nativePC/wp/two/two028/mod/two028.mrl3",
        "nativePC/wp/two/two029/mod/two029.mod3",
        "nativePC/wp/two/two029/mod/two029.mrl3",
    ];
    let (temporary, runtime, package) = fixture(&files);
    assert!(runtime
        .replacement_workflow
        .list_compatible_targets(&GameId::mhw(), &ModId::new("mod-a"), None)
        .is_err());
    let summary = runtime
        .replacement_workflow
        .replacement_summary(
            AnalyzeImportedReplacementRequest {
                game_id: GameId::mhw(),
                mod_id: ModId::new("mod-a"),
            },
            Some(&ProfileId::new("default")),
        )
        .unwrap();
    assert_eq!(summary.sources.len(), 2);
    assert!(summary
        .sources
        .iter()
        .any(|source| source.internal_id == "two028"
            && source.display_names["en"] == "Glacial Demon"));
    assert!(summary
        .sources
        .iter()
        .any(|source| source.internal_id == "two029"
            && source.display_names["en"] == "Fatalis Blade"));
    // 面板只按后端提供的 ID 补充共用模型名称，人工 IPC 必须遵守真实的 ID 对应关系。
    let catalog = MhwReplacementCatalog.replacement_catalog().unwrap();
    for source in &summary.sources {
        let target = catalog
            .targets()
            .iter()
            .find(|target| target.id().as_str() == source.id)
            .expect("weapon source ID also identifies its default catalog target");
        assert_eq!(target.internal_id(), source.internal_id);
        assert_eq!(source.display_names, target.display_name().clone().into());
    }
    assert_no_installation(temporary.path(), &package, &files);
}
