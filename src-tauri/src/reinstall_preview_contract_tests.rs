use super::parse_plan_token;
use crate::reinstall_dto::ReinstallPlanPreviewDto;
use hmm_app::{
    EquipmentRetargetReinstallRequest, ReinstallPlanPreview, ReinstallPreviewStatus,
    RetargetReinstallRequest, StartEquipmentRetargetReinstallTaskRequest,
    StartImportModTaskRequest, StartInstallTaskRequest, StartRetargetReinstallTaskRequest,
    StartUninstallTaskRequest, TaskStatus,
};
use hmm_core::{FileLayer, GameId, InstallTargetPath, ModId, ProfileId, ReplacementTargetId};
use hmm_games_mhw::MhwReplacementCatalog;
use hmm_infra::JsonInstallManifestRepository;
use hmm_ports::{InstallManifestRepository, ReplacementCatalogProvider};
use hmm_runtime::HmmRuntime;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

const WEAPON: &str = "nativePC/wp/one/one001/mod/one001.mod3";
const ARMOR: &str = "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3";
const MODEL_BYTES: &[u8] = b"synthetic unchanged model for preview contract";

struct Fixture {
    _temp: tempfile::TempDir,
    runtime: HmmRuntime,
    app_data: PathBuf,
    game: PathBuf,
    mod_id: ModId,
    baseline: BTreeMap<PathBuf, Vec<u8>>,
}

impl Fixture {
    fn installed(source: &str) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let app_data = temp.path().join("app-data");
        let game = temp.path().join("game");
        fs::create_dir_all(game.join("nativePC/plugins")).unwrap();
        for path in [
            "MonsterHunterWorld.exe",
            "dinput8.dll",
            "loader.dll",
            "nativePC/plugins/MonsterLoader.dll",
            "nativePC/plugins/QuestLoader.dll",
            "nativePC/plugins/!CRCBypass.dll",
        ] {
            fs::write(game.join(path), b"synthetic prerequisite").unwrap();
        }
        fs::write(
            game.join("loader-config.json"),
            br#"{"enablePluginLoader":true}"#,
        )
        .unwrap();
        let baseline = snapshot(&game);
        let archive_path = temp.path().join("preview-contract.zip");
        let mut archive = zip::ZipWriter::new(File::create(&archive_path).unwrap());
        archive
            .start_file(source, SimpleFileOptions::default())
            .unwrap();
        archive.write_all(MODEL_BYTES).unwrap();
        archive.finish().unwrap();
        let runtime = HmmRuntime::from_app_data_dir(app_data.clone()).unwrap();
        runtime
            .game_setup
            .save_game_directory(GameId::mhw(), game.clone())
            .unwrap();
        let task = runtime
            .mod_import_tasks
            .start_import_mod_task(StartImportModTaskRequest {
                archive_path: archive_path.clone(),
            })
            .unwrap();
        runtime
            .mod_import_task_runner
            .run_prepare_task(&task.task_id, archive_path)
            .unwrap();
        let mod_id = ModId::new(task.task_id);
        let request = StartInstallTaskRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: mod_id.clone(),
            layer: FileLayer::new("base", 0),
        };
        let task = runtime
            .install_tasks
            .start_install_task(request.clone())
            .unwrap();
        let events = runtime
            .install_task_runner
            .run_install_task(&task.task_id, request)
            .unwrap();
        assert_eq!(events.last().unwrap().status, TaskStatus::Completed);
        assert_eq!(fs::read(game.join(source)).unwrap(), MODEL_BYTES);
        Self {
            _temp: temp,
            runtime,
            app_data,
            game,
            mod_id,
            baseline,
        }
    }

    fn uninstall_to_baseline(&self) {
        let request = StartUninstallTaskRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: self.mod_id.clone(),
        };
        let task = self
            .runtime
            .uninstall_tasks
            .start_uninstall_task(request.clone())
            .unwrap();
        let events = self
            .runtime
            .uninstall_task_runner
            .run_uninstall_task(&task.task_id, request)
            .unwrap();
        assert_eq!(events.last().unwrap().status, TaskStatus::Completed);
        assert_eq!(snapshot(&self.game), self.baseline);
    }
}

fn preview_token(preview: ReinstallPlanPreview) -> String {
    assert_eq!(preview.status, ReinstallPreviewStatus::Ready);
    assert!(!preview.file_effects.is_empty());
    let dto = ReinstallPlanPreviewDto::try_from(preview).unwrap();
    let wire = serde_json::to_value(dto).unwrap();
    // 使用生产预览、序列化 DTO 和提交解析器，不能用手写合法 token 代替。
    parse_plan_token(wire["planToken"].as_str().unwrap().to_owned())
        .expect("the token produced by the preview must be accepted by the commit command")
}

fn target(internal_id: &str, family: &str) -> ReplacementTargetId {
    MhwReplacementCatalog
        .replacement_catalog()
        .unwrap()
        .targets()
        .iter()
        .find(|target| {
            target.internal_id() == internal_id
                && target
                    .metadata()
                    .get("path_family")
                    .and_then(serde_json::Value::as_str)
                    == Some(family)
        })
        .unwrap()
        .id()
        .clone()
}

#[test]
fn produced_switch_token_crosses_dto_and_commit_parser_for_weapon_and_armor() {
    for (source, destination, internal_id, family) in [
        (
            WEAPON,
            "nativePC/wp/one/one002/mod/one002.mod3",
            "one002",
            "wp/one",
        ),
        (
            ARMOR,
            "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
            "pl129_0000",
            "pl/f_equip",
        ),
    ] {
        let fixture = Fixture::installed(source);
        let before = snapshot(&fixture.game);
        let request = RetargetReinstallRequest {
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            mod_id: fixture.mod_id.clone(),
            target_id: target(internal_id, family),
            layer: FileLayer::new("base", 0),
        };
        let preview = fixture
            .runtime
            .reinstall_executor
            .preview_retarget_reinstall(request.clone())
            .unwrap();
        assert_eq!(
            snapshot(&fixture.game),
            before,
            "preview must not write game files"
        );
        let plan_token = preview_token(preview);
        let repeated = fixture
            .runtime
            .reinstall_executor
            .preview_retarget_reinstall(request.clone())
            .unwrap();
        assert_eq!(preview_token(repeated), plan_token);
        let start = StartRetargetReinstallTaskRequest {
            game_id: request.game_id,
            profile_id: request.profile_id,
            mod_id: request.mod_id,
            target_id: request.target_id,
            layer: request.layer,
            plan_token,
        };
        let task = fixture
            .runtime
            .reinstall_tasks
            .start_retarget_reinstall_task(start.clone())
            .unwrap();
        let events = fixture
            .runtime
            .reinstall_task_runner
            .run_retarget_reinstall_task(&task.task_id, start)
            .unwrap();
        assert_eq!(events.last().unwrap().status, TaskStatus::Completed);
        let mut expected = fixture.baseline.clone();
        expected.insert(PathBuf::from(destination), MODEL_BYTES.to_vec());
        assert_eq!(snapshot(&fixture.game), expected);
        fixture.uninstall_to_baseline();
    }
}

#[test]
fn produced_reapply_token_crosses_dto_and_commit_parser_after_both_extensions() {
    let fixture = Fixture::installed(WEAPON);
    let old_path = "nativePC/wp/one/one001/legacy/one001.mod3";
    fs::create_dir_all(fixture.game.join(old_path).parent().unwrap()).unwrap();
    fs::rename(fixture.game.join(WEAPON), fixture.game.join(old_path)).unwrap();
    let manifests = JsonInstallManifestRepository::new(fixture.app_data.join("install/manifests"));
    let mut manifest = manifests
        .load_manifest(&ProfileId::new("default"))
        .unwrap()
        .unwrap();
    manifest.entries[0].target_path = InstallTargetPath::parse(old_path, ["nativePC"]).unwrap();
    manifests.save_manifest(&manifest).unwrap();
    let before = snapshot(&fixture.game);
    let selection = EquipmentRetargetReinstallRequest::reapply(
        GameId::mhw(),
        ProfileId::new("default"),
        fixture.mod_id.clone(),
    );
    let preview = fixture
        .runtime
        .reinstall_executor
        .preview_equipment_retarget_reinstall(selection.clone())
        .unwrap();
    assert_eq!((preview.counts.added, preview.counts.stale), (1, 1));
    let plan_token = preview_token(preview);
    assert_eq!(snapshot(&fixture.game), before);
    let request = StartEquipmentRetargetReinstallTaskRequest {
        selection: selection.clone(),
        plan_token,
    };
    let task = fixture
        .runtime
        .reinstall_tasks
        .start_equipment_retarget_reinstall_task(request.clone())
        .unwrap();
    let events = fixture
        .runtime
        .reinstall_task_runner
        .run_equipment_retarget_reinstall_task(&task.task_id, request)
        .unwrap();
    assert_eq!(events.last().unwrap().status, TaskStatus::Completed);
    let mut expected = fixture.baseline.clone();
    expected.insert(PathBuf::from(WEAPON), MODEL_BYTES.to_vec());
    assert_eq!(snapshot(&fixture.game), expected);
    let no_changes = fixture
        .runtime
        .reinstall_executor
        .preview_equipment_retarget_reinstall(selection)
        .unwrap();
    assert_eq!(no_changes.status, ReinstallPreviewStatus::NoChanges);
    assert!(no_changes.plan_token.is_none());
    fixture.uninstall_to_baseline();
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                visit(root, &entry.path(), files);
            } else {
                files.insert(
                    entry.path().strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}
