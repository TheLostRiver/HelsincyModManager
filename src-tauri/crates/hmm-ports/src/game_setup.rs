use crate::game_prerequisites::GamePrerequisiteReport;
use hmm_core::{GameDirectoryValidation, GameId, GameInstance};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GameConfigRepositoryError {
    #[error("storage corrupted")]
    StorageCorrupted,
    #[error("storage failed: {0}")]
    StorageFailed(String),
}

pub type GameConfigRepositoryResult<T> = Result<T, GameConfigRepositoryError>;

pub trait GameDirectoryProbe: Send + Sync {
    fn root_dir(&self) -> &Path;
    fn root_exists(&self) -> bool;
    fn exists(&self, relative_path: &str) -> bool;
    fn is_file(&self, relative_path: &str) -> bool;
    fn is_dir(&self, relative_path: &str) -> bool;
    fn read_text_file(&self, relative_path: &str) -> anyhow::Result<String>;
    fn sha256_hex(&self, relative_path: &str) -> anyhow::Result<String>;

    /// 游戏根目录当前是否可写。
    ///
    /// 安装链会覆盖玩家文件，不可写必须在 preflight 就 fail closed，
    /// 而不是等建完 backup、写完 Committing recovery，到第一次真实写入才失败。
    ///
    /// 默认 `true`：只有真实文件系统探针需要回答这个问题，
    /// 纯内存的 fake probe 不受可写性影响。
    fn root_writable(&self) -> bool {
        true
    }
}

pub trait GameDirectoryProbeFactory: Send + Sync {
    fn create(&self, directory: PathBuf) -> Box<dyn GameDirectoryProbe>;
}

pub trait GameAdapter: Send + Sync {
    fn game_id(&self) -> GameId;
    fn display_name(&self) -> &'static str;
    fn validate_directory(&self, probe: &dyn GameDirectoryProbe) -> GameDirectoryValidation;
    fn inspect_prerequisites(&self, probe: &dyn GameDirectoryProbe) -> GamePrerequisiteReport;

    fn allowed_install_roots(&self) -> Vec<String> {
        Vec::new()
    }

    /// 文件名（**只能是路径的最后一段**）是否属于本游戏**绝不安装**的类型。
    ///
    /// 默认 `false`：不是每个游戏都需要这样一份清单。
    ///
    /// 存在的理由是让「这个文件为什么装不了」由**游戏适配器**回答，而不是让上层按扩展名
    /// 硬编码——上层不知道哪些扩展名是某个游戏的正常资源。
    ///
    /// ⚠️ 当前它只描述**能力**，不描述既有链路的行为：MHW 的拒绝清单目前只作用在重定向
    /// 计划产出的文件上，普通安装链路尚未套用（缺口记在 `executable_reject_list.rs` 模块头）。
    /// 因此调用方应当把它当作「命中拒绝清单」这个**事实**来呈现，不要直接等同于
    /// 「不会被安装」。
    fn is_rejected_install_file_name(&self, _file_name: &str) -> bool {
        false
    }

    fn steam_app_id(&self) -> Option<u32> {
        None
    }

    /// 游戏主进程的映像名（如 `MonsterHunterWorld.exe`），供游戏运行检测使用。
    /// 返回空表示该游戏尚不支持运行检测。
    fn process_image_names(&self) -> Vec<String> {
        Vec::new()
    }
}

pub trait GameConfigRepository: Send + Sync {
    fn load_game_instance(
        &self,
        game_id: &GameId,
    ) -> GameConfigRepositoryResult<Option<GameInstance>>;

    fn save_game_instance(&self, instance: &GameInstance) -> GameConfigRepositoryResult<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameDiscoveryRequest {
    pub game_id: GameId,
    pub display_name: String,
    pub steam_app_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameCandidateSource {
    Steam,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameCandidate {
    pub game_id: GameId,
    pub display_name: String,
    pub root_dir: PathBuf,
    pub source: GameCandidateSource,
    pub source_label: String,
}

pub trait GameDiscoveryService: Send + Sync {
    fn scan_candidates(
        &self,
        request: &GameDiscoveryRequest,
    ) -> Result<Vec<GameCandidate>, GameDiscoveryError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GameDiscoveryError {
    #[error("scan not implemented")]
    ScanNotImplemented,
    #[error("scan failed: {0}")]
    ScanFailed(String),
}
