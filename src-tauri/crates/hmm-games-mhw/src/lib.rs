use hmm_core::{
    GameDirectoryEvidence, GameDirectoryEvidenceKind, GameDirectoryValidation, GameId,
    GameSetupErrorCode,
};
use hmm_ports::{GameAdapter, GameDirectoryProbe};

const DISPLAY_NAME: &str = "Monster Hunter: World - Iceborne";
const STEAM_APP_ID: u32 = 582010;
const EXECUTABLE_NAME: &str = "MonsterHunterWorld.exe";
const NATIVE_PC_DIR: &str = "nativePC";

pub struct MonsterHunterWorldAdapter;

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
}

#[cfg(test)]
mod tests {
    use super::MonsterHunterWorldAdapter;
    use hmm_core::{GameDirectoryEvidenceKind, GameSetupErrorCode};
    use hmm_ports::{GameAdapter, GameDirectoryProbe};
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

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
    }

    #[test]
    fn adapter_reports_game_id() {
        let adapter = MonsterHunterWorldAdapter;
        assert_eq!(adapter.game_id().as_str(), "mhw");
    }

    #[test]
    fn adapter_reports_steam_app_id() {
        let adapter = MonsterHunterWorldAdapter;
        assert_eq!(adapter.steam_app_id(), Some(582010));
    }

    #[test]
    fn validates_directory_with_executable() {
        let adapter = MonsterHunterWorldAdapter;
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
        let adapter = MonsterHunterWorldAdapter;
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
        let adapter = MonsterHunterWorldAdapter;
        let probe = FakeProbe::at("C:/Not MHW");

        let validation = adapter.validate_directory(&probe);

        assert!(!validation.is_valid);
        assert_eq!(validation.errors, vec![GameSetupErrorCode::MissingExecutable]);
    }

    #[test]
    fn rejects_missing_root_directory() {
        let adapter = MonsterHunterWorldAdapter;
        let probe = FakeProbe::missing_root("C:/Missing");

        let validation = adapter.validate_directory(&probe);

        assert!(!validation.is_valid);
        assert_eq!(validation.errors, vec![GameSetupErrorCode::DirectoryNotFound]);
    }
}
