use anyhow::Result;
use hmm_core::{
    GameId, ProfileId, SaveDirectoryCandidateConfidence, SaveDirectoryCandidateSummary,
    SteamAccountProfileSummary,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub trait GameSaveDirectoryRule: Send + Sync {
    fn game_id(&self) -> GameId;
    fn steam_app_id(&self) -> u32;
    fn steam_remote_relative_path(&self) -> &'static str;
    fn known_save_file_names(&self) -> &'static [&'static str];
    fn path_label(&self) -> &'static str;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamUserdataScanRequest {
    pub game_id: GameId,
    pub game_root_hint: Option<PathBuf>,
    pub steam_app_id: u32,
    pub remote_relative_path: String,
    pub known_save_file_names: Vec<String>,
    pub path_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedSaveDirectoryCandidate {
    pub candidate_id: String,
    pub account_id_32: u32,
    pub directory: PathBuf,
    pub confidence: SaveDirectoryCandidateConfidence,
    pub last_modified_at: Option<u128>,
    pub evidence: Vec<String>,
    pub account_label: String,
    pub path_label: String,
}

pub trait SteamUserdataScanner: Send + Sync {
    fn scan_save_directories(
        &self,
        request: &SteamUserdataScanRequest,
    ) -> Result<Vec<ScannedSaveDirectoryCandidate>>;

    fn validate_save_directory(
        &self,
        request: &SteamUserdataScanRequest,
        directory: &Path,
    ) -> Result<ScannedSaveDirectoryCandidate>;
}

pub trait SteamAccountProfileClient: Send + Sync {
    fn fetch_profile(
        &self,
        account_id_32: u32,
        timeout: Duration,
    ) -> Result<SteamAccountProfileSummary>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSaveDirectoryCandidate {
    pub summary: SaveDirectoryCandidateSummary,
    pub account_id_32: u32,
    pub directory: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSaveDirectoryDiscovery {
    pub discovery_id: String,
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub expires_at_unix_millis: u128,
    pub candidates: Vec<PendingSaveDirectoryCandidate>,
}

pub trait PendingSaveDirectoryCandidateStore: Send + Sync {
    fn put(&self, discovery: PendingSaveDirectoryDiscovery) -> Result<()>;

    fn get_candidate(
        &self,
        discovery_id: &str,
        candidate_id: &str,
        now_unix_millis: u128,
    ) -> Result<Option<PendingSaveDirectoryCandidate>>;
}
