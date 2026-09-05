use hmm_core::{
    GameDirectoryEvidence, GameDirectoryEvidenceKind, GameDirectoryValidation, GameId,
    GameInstance, GameSetupErrorCode,
};
use hmm_ports::{
    GameAdapter, GameDirectoryProbe, GameLaunchError, GameLaunchMethod, GameLaunchReceipt,
    GameLaunchRunner, GameLauncher, GamePrerequisiteReport, GamePrerequisiteReportState,
    GamePrerequisiteRuleRepository, GamePrerequisiteRuleRepositoryError,
};
use std::sync::Arc;

mod armor_retarget;
mod equipment_catalog_candidate;
mod executable_reject_list;
mod package_path;
mod prerequisites;
mod save_directory;
mod weapon_retarget;

pub use armor_retarget::{
    normalize_armor_display_text, normalize_armor_search_text, ArmorPathError, ArmorResourcePath,
    MhwArmorCatalog, MhwArmorReplacementAdapter,
};
pub use equipment_catalog_candidate::{
    generate_mhw_equipment_stable_id, validate_mhw_equipment_candidate_catalog,
    validate_mhw_equipment_candidate_catalog_for_bundling, EquipmentCandidateBundleBlocker,
    EquipmentCandidateBundlingError, EquipmentCandidateCatalogError,
    EquipmentCandidateIdentityError, EquipmentCandidateTargetKind,
    EquipmentCandidateValidationIssue, EquipmentCandidateValidationReport,
    MHW_EQUIPMENT_CANDIDATE_JSON_SCHEMA, MHW_EQUIPMENT_CANDIDATE_SCHEMA_VERSION,
};
// 拒绝清单不是武器专属概念：防具侧（#336 切片⑥）落地时应复用同一份，
// 所以放在 crate 级而不是 weapon_retarget 里。
pub use executable_reject_list::{
    is_rejected_executable_file_name, MHW_EXECUTABLE_REJECT_EXTENSIONS,
};
pub use save_directory::MonsterHunterWorldSaveDirectoryRule;
pub use weapon_retarget::{
    analyze_mhw_weapon_assets, build_mhw_weapon_mrl3_transform_invocation,
    preflight_mhw_weapon_mod3, preflight_mhw_weapon_model_pair, preflight_mhw_weapon_mrl3,
    transform_mhw_weapon_mrl3_texture_paths, MhwReplacementAdapter, MhwReplacementCatalog,
    MhwWeaponCatalogSource, MhwWeaponMrl3TexturePathTransformer, WeaponAnalysisError,
    WeaponAnalysisWarning, WeaponBinaryError, WeaponCatalogSourceError, WeaponCompanionAsset,
    WeaponCompanionPlacement, WeaponExcludedAsset, WeaponFamily, WeaponFamilyError, WeaponMainId,
    WeaponMod3Preflight, WeaponModelAssetKind, WeaponModelAssetPath, WeaponModelPair,
    WeaponModelPairPreflight, WeaponMrl3Preflight, WeaponMrl3TransformOutput,
    WeaponMrl3TransformReport, WeaponPackageAnalysis, WeaponPartId, WeaponPartRole,
    WeaponPathError, WeaponResourceRoot, WeaponSecondaryPart, WeaponSourceAsset,
    WeaponSourceClosure, WeaponTargetMetadata, WeaponTargetStatus, WeaponUnresolvedModel,
    WeaponUnresolvedModelReason, MHW_WEAPON_BINARY_MAX_BYTES,
    MHW_WEAPON_CATALOG_SOURCE_SCHEMA_VERSION, MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_ID,
    MHW_WEAPON_MRL3_TEXTURE_PATH_TRANSFORMER_VERSION,
};

const DISPLAY_NAME: &str = "Monster Hunter: World - Iceborne";
const STEAM_APP_ID: u32 = 582010;
const EXECUTABLE_NAME: &str = "MonsterHunterWorld.exe";
const NATIVE_PC_DIR: &str = "nativePC";
const BUNDLED_PREREQUISITE_RULES: &str = include_str!("../data/mhw-prerequisites.default.json");

pub struct MonsterHunterWorldAdapter {
    prerequisite_rules: Arc<dyn GamePrerequisiteRuleRepository>,
}

pub struct MonsterHunterWorldLauncher {
    runner: Arc<dyn GameLaunchRunner>,
}

impl MonsterHunterWorldAdapter {
    pub fn new(prerequisite_rules: Arc<dyn GamePrerequisiteRuleRepository>) -> Self {
        Self { prerequisite_rules }
    }
}

impl MonsterHunterWorldLauncher {
    pub fn new(runner: Arc<dyn GameLaunchRunner>) -> Self {
        Self { runner }
    }
}

impl GameLauncher for MonsterHunterWorldLauncher {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn launch(&self, instance: &GameInstance) -> Result<GameLaunchReceipt, GameLaunchError> {
        self.runner
            .open_uri(&format!("steam://rungameid/{STEAM_APP_ID}"))?;

        Ok(GameLaunchReceipt {
            game_id: instance.game_id.clone(),
            method: GameLaunchMethod::SteamProtocol,
        })
    }
}

impl GameAdapter for MonsterHunterWorldAdapter {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn display_name(&self) -> &'static str {
        DISPLAY_NAME
    }

    fn steam_app_id(&self) -> Option<u32> {
        Some(STEAM_APP_ID)
    }

    fn allowed_install_roots(&self) -> Vec<String> {
        vec![NATIVE_PC_DIR.to_owned()]
    }

    /// 与重定向分类器共用同一个判定函数，不另起一份清单——两处得出不同结论会让
    /// 「界面说会被排除、实际装进去了」这种矛盾无声发生。
    fn is_rejected_install_file_name(&self, file_name: &str) -> bool {
        is_rejected_executable_file_name(file_name)
    }

    fn process_image_names(&self) -> Vec<String> {
        vec![EXECUTABLE_NAME.to_owned()]
    }

    fn validate_directory(&self, probe: &dyn GameDirectoryProbe) -> GameDirectoryValidation {
        let mut validation =
            GameDirectoryValidation::new(self.game_id(), probe.root_dir().to_path_buf());

        if !probe.root_exists() {
            validation.confidence = 0;
            validation.add_evidence(GameDirectoryEvidence::new(
                GameDirectoryEvidenceKind::DirectoryMissing,
                "目录不存在",
            ));
            validation.add_error(GameSetupErrorCode::DirectoryNotFound);
            return validation;
        }

        validation.add_evidence(GameDirectoryEvidence::new(
            GameDirectoryEvidenceKind::DirectoryExists,
            "目录存在",
        ));

        if probe.is_file(EXECUTABLE_NAME) {
            validation.confidence = 90;
            validation.add_evidence(GameDirectoryEvidence::new(
                GameDirectoryEvidenceKind::FoundExecutable,
                "找到 MonsterHunterWorld.exe",
            ));
        } else {
            validation.confidence = 20;
            validation.add_evidence(GameDirectoryEvidence::new(
                GameDirectoryEvidenceKind::MissingExecutable,
                "缺少 MonsterHunterWorld.exe",
            ));
            validation.add_error(GameSetupErrorCode::MissingExecutable);
        }

        if probe.is_dir(NATIVE_PC_DIR) {
            validation.confidence = validation.confidence.saturating_add(5).min(100);
            validation.add_evidence(GameDirectoryEvidence::new(
                GameDirectoryEvidenceKind::FoundNativePc,
                "找到 nativePC",
            ));
        }

        validation
    }

    fn inspect_prerequisites(&self, probe: &dyn GameDirectoryProbe) -> GamePrerequisiteReport {
        match self
            .prerequisite_rules
            .load_rules(&self.game_id(), BUNDLED_PREREQUISITE_RULES)
        {
            Ok(rules) => prerequisites::inspect_mhw_prerequisites(probe, rules),
            Err(error) => GamePrerequisiteReport {
                game_id: self.game_id(),
                state: GamePrerequisiteReportState::RulesUnavailable,
                rules_version: None,
                summary_status: None,
                items: Vec::new(),
                error_code: Some(match &error {
                    GamePrerequisiteRuleRepositoryError::StorageFailed(_) => {
                        GameSetupErrorCode::StorageFailed
                    }
                    GamePrerequisiteRuleRepositoryError::StorageCorrupted => {
                        GameSetupErrorCode::StorageCorrupted
                    }
                }),
                message: Some(match &error {
                    GamePrerequisiteRuleRepositoryError::StorageFailed(_) => {
                        "prerequisite rules are unavailable".to_owned()
                    }
                    GamePrerequisiteRuleRepositoryError::StorageCorrupted => {
                        "prerequisite rules are corrupted".to_owned()
                    }
                }),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MonsterHunterWorldAdapter;
    use hmm_core::{GameDirectoryEvidenceKind, GameSetupErrorCode};
    use hmm_ports::{
        GameAdapter, GameDirectoryProbe, GamePrerequisiteRuleRepository,
        GamePrerequisiteRuleRepositoryError, GamePrerequisiteRuleSet,
    };
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    #[derive(Default)]
    struct FakeProbe {
        root_dir: PathBuf,
        root_exists: bool,
        files: HashSet<String>,
        dirs: HashSet<String>,
    }

    impl FakeProbe {
        fn at(root_dir: impl Into<PathBuf>) -> Self {
            Self {
                root_dir: root_dir.into(),
                root_exists: true,
                files: HashSet::new(),
                dirs: HashSet::new(),
            }
        }

        fn missing_root(root_dir: impl Into<PathBuf>) -> Self {
            Self {
                root_dir: root_dir.into(),
                root_exists: false,
                files: HashSet::new(),
                dirs: HashSet::new(),
            }
        }

        fn with_file(mut self, path: &str) -> Self {
            self.files.insert(path.to_owned());
            self
        }

        fn with_dir(mut self, path: &str) -> Self {
            self.dirs.insert(path.to_owned());
            self
        }
    }

    impl GameDirectoryProbe for FakeProbe {
        fn root_dir(&self) -> &Path {
            &self.root_dir
        }

        fn root_exists(&self) -> bool {
            self.root_exists
        }

        fn exists(&self, relative_path: &str) -> bool {
            self.files.contains(relative_path) || self.dirs.contains(relative_path)
        }

        fn is_file(&self, relative_path: &str) -> bool {
            self.files.contains(relative_path)
        }

        fn is_dir(&self, relative_path: &str) -> bool {
            self.dirs.contains(relative_path)
        }

        fn read_text_file(&self, _relative_path: &str) -> hmm_ports::PortResult<String> {
            unreachable!("MHW adapter tests do not read prerequisite text files")
        }

        fn sha256_hex(&self, _relative_path: &str) -> hmm_ports::PortResult<String> {
            unreachable!("MHW adapter tests do not hash prerequisite files")
        }
    }

    struct NoopRuleRepository;

    impl GamePrerequisiteRuleRepository for NoopRuleRepository {
        fn load_rules(
            &self,
            _game_id: &hmm_core::GameId,
            _bundled_default: &str,
        ) -> Result<GamePrerequisiteRuleSet, GamePrerequisiteRuleRepositoryError> {
            panic!("directory validation tests must not load prerequisite rules")
        }
    }

    struct CorruptedRuleRepository;

    impl GamePrerequisiteRuleRepository for CorruptedRuleRepository {
        fn load_rules(
            &self,
            _game_id: &hmm_core::GameId,
            _bundled_default: &str,
        ) -> Result<GamePrerequisiteRuleSet, GamePrerequisiteRuleRepositoryError> {
            Err(GamePrerequisiteRuleRepositoryError::StorageCorrupted)
        }
    }

    fn test_adapter() -> MonsterHunterWorldAdapter {
        MonsterHunterWorldAdapter::new(Arc::new(NoopRuleRepository))
    }

    #[test]
    fn adapter_reports_game_id() {
        let adapter = test_adapter();
        assert_eq!(adapter.game_id().as_str(), "mhw");
    }

    #[test]
    fn adapter_reports_steam_app_id() {
        let adapter = test_adapter();
        assert_eq!(adapter.steam_app_id(), Some(582010));
    }

    #[test]
    fn adapter_reports_allowed_install_roots() {
        let adapter = test_adapter();
        assert_eq!(adapter.allowed_install_roots(), vec!["nativePC".to_owned()]);
    }

    #[test]
    fn adapter_reports_process_image_names() {
        let adapter = test_adapter();
        assert_eq!(
            adapter.process_image_names(),
            vec!["MonsterHunterWorld.exe".to_owned()]
        );
    }

    #[test]
    fn validates_directory_with_executable() {
        let adapter = test_adapter();
        let probe = FakeProbe::at("C:/Monster Hunter World").with_file("MonsterHunterWorld.exe");

        let validation = adapter.validate_directory(&probe);

        assert!(validation.is_valid);
        assert_eq!(validation.errors, Vec::<GameSetupErrorCode>::new());
        assert!(validation
            .evidence
            .iter()
            .any(|item| item.kind == GameDirectoryEvidenceKind::FoundExecutable));
    }

    #[test]
    fn native_pc_is_evidence_but_not_required() {
        let adapter = test_adapter();
        let probe = FakeProbe::at("C:/Monster Hunter World")
            .with_file("MonsterHunterWorld.exe")
            .with_dir("nativePC");

        let validation = adapter.validate_directory(&probe);

        assert!(validation.is_valid);
        assert!(validation
            .evidence
            .iter()
            .any(|item| item.kind == GameDirectoryEvidenceKind::FoundNativePc));
    }

    #[test]
    fn rejects_directory_missing_executable() {
        let adapter = test_adapter();
        let probe = FakeProbe::at("C:/Not MHW");

        let validation = adapter.validate_directory(&probe);

        assert!(!validation.is_valid);
        assert_eq!(
            validation.errors,
            vec![GameSetupErrorCode::MissingExecutable]
        );
    }

    #[test]
    fn rejects_missing_root_directory() {
        let adapter = test_adapter();
        let probe = FakeProbe::missing_root("C:/Missing");

        let validation = adapter.validate_directory(&probe);

        assert!(!validation.is_valid);
        assert_eq!(
            validation.errors,
            vec![GameSetupErrorCode::DirectoryNotFound]
        );
    }

    #[test]
    fn prerequisite_report_returns_rules_unavailable_when_repository_is_corrupted() {
        let adapter = MonsterHunterWorldAdapter::new(Arc::new(CorruptedRuleRepository));
        let probe = FakeProbe::at("C:/Monster Hunter World");

        let report = adapter.inspect_prerequisites(&probe);

        assert_eq!(
            report.state,
            hmm_ports::GamePrerequisiteReportState::RulesUnavailable
        );
        assert_eq!(
            report.error_code,
            Some(GameSetupErrorCode::StorageCorrupted)
        );
    }
}
