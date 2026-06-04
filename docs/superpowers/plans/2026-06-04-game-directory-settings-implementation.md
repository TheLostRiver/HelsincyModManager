# Game Directory Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 落地《怪物猎人：世界 冰原》首次启动游戏目录配置闭环：手动选择目录、后端校验、JSON 保存配置、Dashboard 展示真实状态。

**Architecture:** 以 `hmm-core -> hmm-ports -> hmm-games-mhw / hmm-infra -> hmm-app -> Tauri commands -> React feature` 的方向推进。游戏目录规则只放在 MHW:I adapter，真实文件系统只放在 infra，Dashboard 只消费 `features/game-setup` 提供的状态和动作。

**Tech Stack:** Tauri 2、React 19、TypeScript、Rust workspace、serde/serde_json、thiserror、`@tauri-apps/plugin-dialog`、`tauri-plugin-dialog`、Vite、lucide-react。

---

## Scope

本计划实现：

- 手动选择 MHW:I 游戏目录。
- 校验玩家选择的目录是否包含 `MonsterHunterWorld.exe`。
- 将已校验通过的目录保存到 Tauri app data 下的 `config/games.json`。
- Dashboard 根据真实状态显示未配置、校验中、校验失败、已配置。
- `scan_game_candidates("mhw")` 返回明确的 `scan_not_implemented`，用于 UI 告知自动扫描尚未启用。

本计划不实现：

- 真实 Steam library 扫描。
- 进程扫描。
- 一键启动游戏。
- Mod 导入、安装、卸载、备份、回滚。
- 写入真实游戏目录。
- 读取真实存档。
- SQLite migration。

## Target File Structure

```text
src/
  features/
    game-setup/
      GameDirectoryActions.tsx
      gameSetupApi.ts
      gameSetupTypes.ts
      gameSetupViewModel.ts
      useGameSetup.ts
    dashboard/
      DashboardHeroCard.tsx
      DashboardPage.tsx
      SetupStatusPanel.tsx
      dashboardData.ts
  shared/
    api/
      tauri.ts

src-tauri/
  Cargo.toml
  capabilities/default.json
  src/
    dto.rs
    game_setup_commands.rs
    lib.rs
    state.rs
  crates/
    hmm-core/src/
      game.rs
      lib.rs
    hmm-ports/src/
      game_setup.rs
      lib.rs
    hmm-games-mhw/src/lib.rs
    hmm-app/src/
      game_setup.rs
      lib.rs
    hmm-infra/src/
      game_config_repository.rs
      game_directory_probe.rs
      game_discovery.rs
      lib.rs
```

职责锁定：

- `hmm-core`：领域数据结构和错误码，不访问文件系统。
- `hmm-ports`：trait 边界，不包含 JSON、Steam、Tauri 细节。
- `hmm-games-mhw`：只写 MHW:I 目录识别规则。
- `hmm-infra`：真实文件系统 probe、JSON repository、扫描未启用服务。
- `hmm-app`：编排用例，只依赖 trait。
- `src-tauri/src/*`：Tauri 状态、DTO 和 command 映射。
- `features/game-setup`：前端 typed API、状态 hook、目录动作。
- `features/dashboard`：展示状态，不直接调用 `invoke`，不写游戏文件规则。

## Task 0: Preflight

**Files:**

- Read: `AGENTS.md`
- Read: `docs/superpowers/specs/2026-06-04-game-directory-settings-design.md`
- Read: `docs/TESTING.md`
- Read: `src-tauri/capabilities/default.json`
- Read: `package.json`
- Read: `Cargo.toml`

- [ ] **Step 1: Confirm branch and clean tree**

Run:

```powershell
git status --short --branch
```

Expected:

```text
## codex/game-directory-settings-implementation
```

There must be no unrelated modified or untracked files before implementation starts.

- [ ] **Step 2: Confirm current dependencies**

Run:

```powershell
Get-Content -Path package.json -Encoding UTF8
Get-Content -Path Cargo.toml -Encoding UTF8
Get-Content -Path src-tauri/Cargo.toml -Encoding UTF8
Get-Content -Path src-tauri/capabilities/default.json -Encoding UTF8
```

Expected:

- `@tauri-apps/plugin-dialog` is not present yet.
- `tauri-plugin-dialog` is not present yet.
- `src-tauri/capabilities/default.json` currently contains `core:default`.

## Task 1: Core Domain Models

**Files:**

- Create: `src-tauri/crates/hmm-core/src/game.rs`
- Modify: `src-tauri/crates/hmm-core/src/lib.rs`
- Test: `cargo test -p hmm-core`

- [ ] **Step 1: Write failing core tests**

Create `src-tauri/crates/hmm-core/src/game.rs` with the public types and tests below. The tests define the expected shape first.

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

pub const MHW_GAME_ID: &str = "mhw";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GameIdError {
    #[error("game id cannot be empty")]
    Empty,
    #[error("unsupported game id: {0}")]
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GameId(String);

impl GameId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, GameIdError> {
        let value = value.into();
        let trimmed = value.trim();

        if trimmed.is_empty() {
            return Err(GameIdError::Empty);
        }

        if trimmed != MHW_GAME_ID {
            return Err(GameIdError::Unsupported(trimmed.to_owned()));
        }

        Ok(Self(trimmed.to_owned()))
    }

    pub fn mhw() -> Self {
        Self(MHW_GAME_ID.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameDirectoryStatus {
    NotConfigured,
    Invalid,
    Configured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameDirectoryEvidenceKind {
    DirectoryExists,
    DirectoryMissing,
    FoundExecutable,
    MissingExecutable,
    FoundNativePc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameDirectoryEvidence {
    pub kind: GameDirectoryEvidenceKind,
    pub label: String,
}

impl GameDirectoryEvidence {
    pub fn new(kind: GameDirectoryEvidenceKind, label: impl Into<String>) -> Self {
        Self {
            kind,
            label: label.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameSetupErrorCode {
    UnsupportedGame,
    DirectoryNotFound,
    MissingExecutable,
    StorageFailed,
    StorageCorrupted,
    ScanNotImplemented,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameDirectoryValidation {
    pub game_id: GameId,
    pub directory: PathBuf,
    pub is_valid: bool,
    pub confidence: u8,
    pub evidence: Vec<GameDirectoryEvidence>,
    pub errors: Vec<GameSetupErrorCode>,
}

impl GameDirectoryValidation {
    pub fn new(game_id: GameId, directory: PathBuf) -> Self {
        Self {
            game_id,
            directory,
            is_valid: true,
            confidence: 0,
            evidence: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn add_evidence(&mut self, evidence: GameDirectoryEvidence) {
        self.evidence.push(evidence);
    }

    pub fn add_error(&mut self, error: GameSetupErrorCode) {
        self.is_valid = false;
        self.errors.push(error);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameInstance {
    pub id: String,
    pub game_id: GameId,
    pub display_name: String,
    pub root_dir: PathBuf,
    pub status: GameDirectoryStatus,
    pub configured_at_unix_millis: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSetupStatus {
    pub game_id: GameId,
    pub status: GameDirectoryStatus,
    pub instance: Option<GameInstance>,
    pub error_code: Option<GameSetupErrorCode>,
    pub message: Option<String>,
}

impl GameSetupStatus {
    pub fn not_configured(game_id: GameId) -> Self {
        Self {
            game_id,
            status: GameDirectoryStatus::NotConfigured,
            instance: None,
            error_code: None,
            message: None,
        }
    }

    pub fn configured(instance: GameInstance) -> Self {
        Self {
            game_id: instance.game_id.clone(),
            status: GameDirectoryStatus::Configured,
            instance: Some(instance),
            error_code: None,
            message: None,
        }
    }

    pub fn invalid(game_id: GameId, error_code: GameSetupErrorCode, message: impl Into<String>) -> Self {
        Self {
            game_id,
            status: GameDirectoryStatus::Invalid,
            instance: None,
            error_code: Some(error_code),
            message: Some(message.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_game_id() {
        let id = GameId::parse("mhw").expect("mhw should be supported");
        assert_eq!(id.as_str(), "mhw");
    }

    #[test]
    fn rejects_empty_game_id() {
        let result = GameId::parse(" ");
        assert_eq!(result, Err(GameIdError::Empty));
    }

    #[test]
    fn rejects_unsupported_game_id() {
        let result = GameId::parse("rise");
        assert_eq!(result, Err(GameIdError::Unsupported("rise".to_owned())));
    }

    #[test]
    fn validation_becomes_invalid_after_error() {
        let mut validation = GameDirectoryValidation::new(GameId::mhw(), PathBuf::from("C:/Game"));

        validation.add_error(GameSetupErrorCode::MissingExecutable);

        assert!(!validation.is_valid);
        assert_eq!(validation.errors, vec![GameSetupErrorCode::MissingExecutable]);
    }

    #[test]
    fn configured_status_wraps_instance() {
        let instance = GameInstance {
            id: "mhw-default".to_owned(),
            game_id: GameId::mhw(),
            display_name: "Monster Hunter: World - Iceborne".to_owned(),
            root_dir: PathBuf::from("C:/Game"),
            status: GameDirectoryStatus::Configured,
            configured_at_unix_millis: 1,
        };

        let status = GameSetupStatus::configured(instance);

        assert_eq!(status.status, GameDirectoryStatus::Configured);
        assert!(status.instance.is_some());
    }
}
```

- [ ] **Step 2: Export the module**

Replace `src-tauri/crates/hmm-core/src/lib.rs` with:

```rust
mod game;

pub use game::{
    GameDirectoryEvidence, GameDirectoryEvidenceKind, GameDirectoryStatus, GameDirectoryValidation, GameId,
    GameIdError, GameInstance, GameSetupErrorCode, GameSetupStatus, MHW_GAME_ID,
};
```

- [ ] **Step 3: Run core tests and confirm failure or compile issues are limited to new API consumers**

Run:

```powershell
cargo test -p hmm-core
```

Expected:

```text
test result: ok
```

If workspace consumers fail later because they expect the old inline `GameId` definition, update those consumers in Task 3.

- [ ] **Step 4: Commit core domain models**

Run:

```powershell
git add src-tauri/crates/hmm-core/src/game.rs src-tauri/crates/hmm-core/src/lib.rs
git commit -m "feat: 添加游戏目录配置领域模型"
```

## Task 2: Ports for Game Setup

**Files:**

- Create: `src-tauri/crates/hmm-ports/src/game_setup.rs`
- Modify: `src-tauri/crates/hmm-ports/src/lib.rs`
- Modify: `src-tauri/crates/hmm-ports/Cargo.toml`
- Test: `cargo test -p hmm-ports`

- [ ] **Step 1: Add typed port errors and traits**

Create `src-tauri/crates/hmm-ports/src/game_setup.rs`:

```rust
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
}

pub trait GameDirectoryProbeFactory: Send + Sync {
    fn create(&self, directory: PathBuf) -> Box<dyn GameDirectoryProbe>;
}

pub trait GameAdapter: Send + Sync {
    fn game_id(&self) -> GameId;
    fn display_name(&self) -> &'static str;
    fn validate_directory(&self, probe: &dyn GameDirectoryProbe) -> GameDirectoryValidation;
}

pub trait GameConfigRepository: Send + Sync {
    fn load_game_instance(&self, game_id: &GameId) -> GameConfigRepositoryResult<Option<GameInstance>>;
    fn save_game_instance(&self, instance: &GameInstance) -> GameConfigRepositoryResult<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameCandidate {
    pub game_id: GameId,
    pub display_name: String,
    pub root_dir: PathBuf,
    pub source: String,
}

pub trait GameDiscoveryService: Send + Sync {
    fn scan_candidates(&self, game_id: &GameId) -> Result<Vec<GameCandidate>, GameDiscoveryError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GameDiscoveryError {
    #[error("scan not implemented")]
    ScanNotImplemented,
    #[error("scan failed: {0}")]
    ScanFailed(String),
}
```

- [ ] **Step 2: Export ports and keep AppClock**

Replace `src-tauri/crates/hmm-ports/src/lib.rs` with:

```rust
mod game_setup;

use anyhow::Result;

pub use game_setup::{
    GameAdapter, GameCandidate, GameConfigRepository, GameConfigRepositoryError, GameConfigRepositoryResult,
    GameDirectoryProbe, GameDirectoryProbeFactory, GameDiscoveryError, GameDiscoveryService,
};

pub trait AppClock: Send + Sync {
    fn now_unix_millis(&self) -> Result<u128>;
}
```

- [ ] **Step 3: Add thiserror to ports crate**

Modify `src-tauri/crates/hmm-ports/Cargo.toml`:

```toml
[dependencies]
anyhow.workspace = true
hmm-core = { path = "../hmm-core" }
thiserror.workspace = true
```

- [ ] **Step 4: Run ports tests**

Run:

```powershell
cargo test -p hmm-ports
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit ports**

Run:

```powershell
git add src-tauri/crates/hmm-ports
git commit -m "feat: 添加游戏目录配置接口层"
```

## Task 3: MHW:I Adapter Directory Validation

**Files:**

- Modify: `src-tauri/crates/hmm-games-mhw/src/lib.rs`
- Test: `cargo test -p hmm-games-mhw`

- [ ] **Step 1: Replace adapter with validation tests and implementation**

Replace `src-tauri/crates/hmm-games-mhw/src/lib.rs` with:

```rust
use hmm_core::{
    GameDirectoryEvidence, GameDirectoryEvidenceKind, GameDirectoryValidation, GameId, GameSetupErrorCode,
};
use hmm_ports::{GameAdapter, GameDirectoryProbe};

const DISPLAY_NAME: &str = "Monster Hunter: World - Iceborne";
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

    fn validate_directory(&self, probe: &dyn GameDirectoryProbe) -> GameDirectoryValidation {
        let mut validation = GameDirectoryValidation::new(self.game_id(), probe.root_dir().to_path_buf());

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
```

- [ ] **Step 2: Run MHW adapter tests**

Run:

```powershell
cargo test -p hmm-games-mhw
```

Expected:

```text
test result: ok
```

- [ ] **Step 3: Commit MHW adapter**

Run:

```powershell
git add src-tauri/crates/hmm-games-mhw/src/lib.rs
git commit -m "feat: 添加 MHW 游戏目录校验规则"
```

## Task 4: Application Use Cases

**Files:**

- Create: `src-tauri/crates/hmm-app/src/game_setup.rs`
- Modify: `src-tauri/crates/hmm-app/src/lib.rs`
- Modify: `src-tauri/crates/hmm-app/Cargo.toml`
- Test: `cargo test -p hmm-app`

- [ ] **Step 1: Add app service tests and implementation**

Create `src-tauri/crates/hmm-app/src/game_setup.rs`:

```rust
use hmm_core::{GameDirectoryStatus, GameDirectoryValidation, GameId, GameInstance, GameSetupErrorCode, GameSetupStatus};
use hmm_ports::{
    AppClock, GameAdapter, GameConfigRepository, GameConfigRepositoryError, GameDirectoryProbeFactory,
    GameDiscoveryError, GameDiscoveryService,
};
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GameSetupServiceError {
    #[error("unsupported game")]
    UnsupportedGame,
    #[error("directory validation failed")]
    ValidationFailed(GameDirectoryValidation),
    #[error("storage corrupted")]
    StorageCorrupted,
    #[error("storage failed: {0}")]
    StorageFailed(String),
    #[error("scan not implemented")]
    ScanNotImplemented,
    #[error("clock failed: {0}")]
    ClockFailed(String),
}

pub struct GameSetupService {
    adapters: Vec<Arc<dyn GameAdapter>>,
    repository: Arc<dyn GameConfigRepository>,
    probe_factory: Arc<dyn GameDirectoryProbeFactory>,
    discovery: Arc<dyn GameDiscoveryService>,
    clock: Arc<dyn AppClock>,
}

impl GameSetupService {
    pub fn new(
        adapters: Vec<Arc<dyn GameAdapter>>,
        repository: Arc<dyn GameConfigRepository>,
        probe_factory: Arc<dyn GameDirectoryProbeFactory>,
        discovery: Arc<dyn GameDiscoveryService>,
        clock: Arc<dyn AppClock>,
    ) -> Self {
        Self {
            adapters,
            repository,
            probe_factory,
            discovery,
            clock,
        }
    }

    pub fn get_status(&self, game_id: GameId) -> Result<GameSetupStatus, GameSetupServiceError> {
        self.require_adapter(&game_id)?;

        let instance = self.repository.load_game_instance(&game_id).map_err(Self::map_storage_error)?;

        Ok(match instance {
            Some(instance) => GameSetupStatus::configured(instance),
            None => GameSetupStatus::not_configured(game_id),
        })
    }

    pub fn validate_directory(
        &self,
        game_id: GameId,
        directory: PathBuf,
    ) -> Result<GameDirectoryValidation, GameSetupServiceError> {
        let adapter = self.require_adapter(&game_id)?;
        let probe = self.probe_factory.create(directory);
        Ok(adapter.validate_directory(probe.as_ref()))
    }

    pub fn save_game_directory(&self, game_id: GameId, directory: PathBuf) -> Result<GameSetupStatus, GameSetupServiceError> {
        let adapter = self.require_adapter(&game_id)?;
        let validation = self.validate_directory(game_id.clone(), directory.clone())?;

        if !validation.is_valid {
            return Err(GameSetupServiceError::ValidationFailed(validation));
        }

        let instance = GameInstance {
            id: format!("{}-default", game_id.as_str()),
            game_id,
            display_name: adapter.display_name().to_owned(),
            root_dir: directory,
            status: GameDirectoryStatus::Configured,
            configured_at_unix_millis: self
                .clock
                .now_unix_millis()
                .map_err(|error| GameSetupServiceError::ClockFailed(error.to_string()))?,
        };

        self.repository
            .save_game_instance(&instance)
            .map_err(Self::map_storage_error)?;

        Ok(GameSetupStatus::configured(instance))
    }

    pub fn scan_candidates(&self, game_id: GameId) -> Result<(), GameSetupServiceError> {
        self.require_adapter(&game_id)?;
        self.discovery.scan_candidates(&game_id).map(|_| ()).map_err(|error| match error {
            GameDiscoveryError::ScanNotImplemented => GameSetupServiceError::ScanNotImplemented,
            GameDiscoveryError::ScanFailed(message) => GameSetupServiceError::StorageFailed(message),
        })
    }

    fn require_adapter(&self, game_id: &GameId) -> Result<Arc<dyn GameAdapter>, GameSetupServiceError> {
        self.adapters
            .iter()
            .find(|adapter| adapter.game_id() == *game_id)
            .cloned()
            .ok_or(GameSetupServiceError::UnsupportedGame)
    }

    fn map_storage_error(error: GameConfigRepositoryError) -> GameSetupServiceError {
        match error {
            GameConfigRepositoryError::StorageCorrupted => GameSetupServiceError::StorageCorrupted,
            GameConfigRepositoryError::StorageFailed(message) => GameSetupServiceError::StorageFailed(message),
        }
    }
}

impl GameSetupServiceError {
    pub fn error_code(&self) -> GameSetupErrorCode {
        match self {
            Self::UnsupportedGame => GameSetupErrorCode::UnsupportedGame,
            Self::ValidationFailed(validation) => validation
                .errors
                .first()
                .cloned()
                .unwrap_or(GameSetupErrorCode::Unknown),
            Self::StorageCorrupted => GameSetupErrorCode::StorageCorrupted,
            Self::StorageFailed(_) => GameSetupErrorCode::StorageFailed,
            Self::ScanNotImplemented => GameSetupErrorCode::ScanNotImplemented,
            Self::ClockFailed(_) => GameSetupErrorCode::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{GameDirectoryEvidence, GameDirectoryEvidenceKind};
    use hmm_ports::{
        GameConfigRepositoryResult, GameDirectoryProbe, GameDiscoveryService, GameCandidate,
    };
    use std::path::Path;
    use std::sync::Mutex;

    struct FakeClock;

    impl AppClock for FakeClock {
        fn now_unix_millis(&self) -> anyhow::Result<u128> {
            Ok(42)
        }
    }

    struct FakeRepository {
        stored: Mutex<Option<GameInstance>>,
    }

    impl FakeRepository {
        fn empty() -> Self {
            Self {
                stored: Mutex::new(None),
            }
        }
    }

    impl GameConfigRepository for FakeRepository {
        fn load_game_instance(&self, _game_id: &GameId) -> GameConfigRepositoryResult<Option<GameInstance>> {
            Ok(self.stored.lock().expect("fake repo lock").clone())
        }

        fn save_game_instance(&self, instance: &GameInstance) -> GameConfigRepositoryResult<()> {
            *self.stored.lock().expect("fake repo lock") = Some(instance.clone());
            Ok(())
        }
    }

    struct FakeProbe {
        root_dir: PathBuf,
    }

    impl GameDirectoryProbe for FakeProbe {
        fn root_dir(&self) -> &Path {
            &self.root_dir
        }

        fn root_exists(&self) -> bool {
            true
        }

        fn exists(&self, _relative_path: &str) -> bool {
            true
        }

        fn is_file(&self, _relative_path: &str) -> bool {
            true
        }

        fn is_dir(&self, _relative_path: &str) -> bool {
            false
        }
    }

    struct FakeProbeFactory;

    impl GameDirectoryProbeFactory for FakeProbeFactory {
        fn create(&self, directory: PathBuf) -> Box<dyn GameDirectoryProbe> {
            Box::new(FakeProbe { root_dir: directory })
        }
    }

    struct FakeAdapter {
        valid: bool,
    }

    impl GameAdapter for FakeAdapter {
        fn game_id(&self) -> GameId {
            GameId::mhw()
        }

        fn display_name(&self) -> &'static str {
            "Monster Hunter: World - Iceborne"
        }

        fn validate_directory(&self, probe: &dyn GameDirectoryProbe) -> GameDirectoryValidation {
            let mut validation = GameDirectoryValidation::new(self.game_id(), probe.root_dir().to_path_buf());
            validation.add_evidence(GameDirectoryEvidence::new(
                GameDirectoryEvidenceKind::DirectoryExists,
                "目录存在",
            ));
            if !self.valid {
                validation.add_error(GameSetupErrorCode::MissingExecutable);
            }
            validation
        }
    }

    struct NoopDiscovery;

    impl GameDiscoveryService for NoopDiscovery {
        fn scan_candidates(&self, _game_id: &GameId) -> Result<Vec<GameCandidate>, GameDiscoveryError> {
            Err(GameDiscoveryError::ScanNotImplemented)
        }
    }

    fn service_with(adapter: FakeAdapter) -> GameSetupService {
        GameSetupService::new(
            vec![Arc::new(adapter)],
            Arc::new(FakeRepository::empty()),
            Arc::new(FakeProbeFactory),
            Arc::new(NoopDiscovery),
            Arc::new(FakeClock),
        )
    }

    #[test]
    fn status_is_not_configured_without_saved_instance() {
        let service = service_with(FakeAdapter { valid: true });

        let status = service.get_status(GameId::mhw()).expect("status should load");

        assert_eq!(status.status, GameDirectoryStatus::NotConfigured);
    }

    #[test]
    fn save_directory_validates_before_persisting() {
        let service = service_with(FakeAdapter { valid: true });

        let status = service
            .save_game_directory(GameId::mhw(), PathBuf::from("C:/MHW"))
            .expect("valid directory should save");

        assert_eq!(status.status, GameDirectoryStatus::Configured);
        assert_eq!(status.instance.expect("instance").configured_at_unix_millis, 42);
    }

    #[test]
    fn save_directory_rejects_invalid_validation() {
        let service = service_with(FakeAdapter { valid: false });

        let error = service
            .save_game_directory(GameId::mhw(), PathBuf::from("C:/Wrong"))
            .expect_err("invalid directory should fail");

        assert_eq!(error.error_code(), GameSetupErrorCode::MissingExecutable);
    }

    #[test]
    fn scan_candidates_returns_explicit_not_implemented() {
        let service = service_with(FakeAdapter { valid: true });

        let error = service
            .scan_candidates(GameId::mhw())
            .expect_err("scan should be disabled in first version");

        assert_eq!(error.error_code(), GameSetupErrorCode::ScanNotImplemented);
    }
}
```

- [ ] **Step 2: Export app service**

Replace `src-tauri/crates/hmm-app/src/lib.rs` with:

```rust
mod game_setup;

pub use game_setup::{GameSetupService, GameSetupServiceError};

pub fn app_name() -> &'static str {
    "Helsincy Mod Manager"
}

#[cfg(test)]
mod tests {
    use super::app_name;

    #[test]
    fn app_name_is_stable() {
        assert_eq!(app_name(), "Helsincy Mod Manager");
    }
}
```

- [ ] **Step 3: Add thiserror to app crate**

Modify `src-tauri/crates/hmm-app/Cargo.toml`:

```toml
[dependencies]
anyhow.workspace = true
hmm-core = { path = "../hmm-core" }
hmm-ports = { path = "../hmm-ports" }
thiserror.workspace = true
```

- [ ] **Step 4: Run app tests**

Run:

```powershell
cargo test -p hmm-app
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit app service**

Run:

```powershell
git add src-tauri/crates/hmm-app
git commit -m "feat: 添加游戏目录配置应用用例"
```

## Task 5: Infrastructure Repository, Probe, and Disabled Discovery

**Files:**

- Create: `src-tauri/crates/hmm-infra/src/game_config_repository.rs`
- Create: `src-tauri/crates/hmm-infra/src/game_directory_probe.rs`
- Create: `src-tauri/crates/hmm-infra/src/game_discovery.rs`
- Modify: `src-tauri/crates/hmm-infra/src/lib.rs`
- Modify: `src-tauri/crates/hmm-infra/Cargo.toml`
- Test: `cargo test -p hmm-infra`

- [ ] **Step 1: Add JSON repository tests and implementation**

Create `src-tauri/crates/hmm-infra/src/game_config_repository.rs`:

```rust
use hmm_core::{GameId, GameInstance};
use hmm_ports::{GameConfigRepository, GameConfigRepositoryError, GameConfigRepositoryResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
struct GamesConfigFile {
    version: u32,
    games: Vec<GameInstance>,
}

impl Default for GamesConfigFile {
    fn default() -> Self {
        Self {
            version: 1,
            games: Vec::new(),
        }
    }
}

pub struct JsonGameConfigRepository {
    file_path: PathBuf,
}

impl JsonGameConfigRepository {
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    fn load_file(&self) -> GameConfigRepositoryResult<GamesConfigFile> {
        if !self.file_path.exists() {
            return Ok(GamesConfigFile::default());
        }

        let content = fs::read_to_string(&self.file_path)
            .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;

        serde_json::from_str(&content).map_err(|_| GameConfigRepositoryError::StorageCorrupted)
    }

    fn save_file(&self, config: &GamesConfigFile) -> GameConfigRepositoryResult<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;
        }

        let serialized = serde_json::to_string_pretty(config)
            .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;
        let temp_path = self.file_path.with_extension("json.tmp");

        fs::write(&temp_path, serialized).map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;
        fs::rename(&temp_path, &self.file_path)
            .map_err(|error| GameConfigRepositoryError::StorageFailed(error.to_string()))?;

        Ok(())
    }
}

impl GameConfigRepository for JsonGameConfigRepository {
    fn load_game_instance(&self, game_id: &GameId) -> GameConfigRepositoryResult<Option<GameInstance>> {
        let config = self.load_file()?;
        Ok(config.games.into_iter().find(|instance| instance.game_id == *game_id))
    }

    fn save_game_instance(&self, instance: &GameInstance) -> GameConfigRepositoryResult<()> {
        let mut config = self.load_file()?;
        config.games.retain(|item| item.game_id != instance.game_id);
        config.games.push(instance.clone());
        self.save_file(&config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{GameDirectoryStatus, GameId};

    fn test_file(name: &str) -> PathBuf {
        let unique = format!(
            "hmm-json-repo-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        std::env::temp_dir().join(unique).join("config").join("games.json")
    }

    fn instance(root: &str) -> GameInstance {
        GameInstance {
            id: "mhw-default".to_owned(),
            game_id: GameId::mhw(),
            display_name: "Monster Hunter: World - Iceborne".to_owned(),
            root_dir: PathBuf::from(root),
            status: GameDirectoryStatus::Configured,
            configured_at_unix_millis: 42,
        }
    }

    #[test]
    fn missing_file_loads_empty_config() {
        let repo = JsonGameConfigRepository::new(test_file("missing"));

        let loaded = repo.load_game_instance(&GameId::mhw()).expect("load should succeed");

        assert!(loaded.is_none());
    }

    #[test]
    fn save_creates_parent_directory_and_loads_instance() {
        let path = test_file("save");
        let repo = JsonGameConfigRepository::new(path);

        repo.save_game_instance(&instance("C:/MHW")).expect("save should succeed");
        let loaded = repo.load_game_instance(&GameId::mhw()).expect("load should succeed");

        assert_eq!(loaded.expect("instance").root_dir, PathBuf::from("C:/MHW"));
    }

    #[test]
    fn corrupted_json_returns_storage_corrupted() {
        let path = test_file("corrupted");
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(&path, "{ broken json").expect("write broken file");
        let repo = JsonGameConfigRepository::new(path);

        let error = repo.load_game_instance(&GameId::mhw()).expect_err("broken json should fail");

        assert_eq!(error, GameConfigRepositoryError::StorageCorrupted);
    }

    #[test]
    fn save_replaces_existing_game_instance() {
        let path = test_file("replace");
        let repo = JsonGameConfigRepository::new(path);

        repo.save_game_instance(&instance("C:/Old")).expect("first save");
        repo.save_game_instance(&instance("D:/New")).expect("second save");
        let loaded = repo.load_game_instance(&GameId::mhw()).expect("load should succeed");

        assert_eq!(loaded.expect("instance").root_dir, PathBuf::from("D:/New"));
    }
}
```

- [ ] **Step 2: Add real directory probe**

Create `src-tauri/crates/hmm-infra/src/game_directory_probe.rs`:

```rust
use hmm_ports::{GameDirectoryProbe, GameDirectoryProbeFactory};
use std::path::{Path, PathBuf};

pub struct RealGameDirectoryProbe {
    root_dir: PathBuf,
}

impl RealGameDirectoryProbe {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    fn join_relative(&self, relative_path: &str) -> PathBuf {
        self.root_dir.join(relative_path)
    }
}

impl GameDirectoryProbe for RealGameDirectoryProbe {
    fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    fn root_exists(&self) -> bool {
        self.root_dir.is_dir()
    }

    fn exists(&self, relative_path: &str) -> bool {
        self.join_relative(relative_path).exists()
    }

    fn is_file(&self, relative_path: &str) -> bool {
        self.join_relative(relative_path).is_file()
    }

    fn is_dir(&self, relative_path: &str) -> bool {
        self.join_relative(relative_path).is_dir()
    }
}

pub struct RealGameDirectoryProbeFactory;

impl GameDirectoryProbeFactory for RealGameDirectoryProbeFactory {
    fn create(&self, directory: PathBuf) -> Box<dyn GameDirectoryProbe> {
        Box::new(RealGameDirectoryProbe::new(directory))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn probe_checks_files_relative_to_root() {
        let root = std::env::temp_dir().join(format!(
            "hmm-probe-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("nativePC")).expect("create dir");
        fs::write(root.join("MonsterHunterWorld.exe"), b"fake exe").expect("write file");

        let probe = RealGameDirectoryProbe::new(root);

        assert!(probe.root_exists());
        assert!(probe.is_file("MonsterHunterWorld.exe"));
        assert!(probe.is_dir("nativePC"));
    }
}
```

- [ ] **Step 3: Add disabled discovery service**

Create `src-tauri/crates/hmm-infra/src/game_discovery.rs`:

```rust
use hmm_core::GameId;
use hmm_ports::{GameCandidate, GameDiscoveryError, GameDiscoveryService};

pub struct NoopGameDiscoveryService;

impl GameDiscoveryService for NoopGameDiscoveryService {
    fn scan_candidates(&self, _game_id: &GameId) -> Result<Vec<GameCandidate>, GameDiscoveryError> {
        Err(GameDiscoveryError::ScanNotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_returns_explicit_not_implemented() {
        let service = NoopGameDiscoveryService;

        let error = service.scan_candidates(&GameId::mhw()).expect_err("scan is disabled");

        assert_eq!(error, GameDiscoveryError::ScanNotImplemented);
    }
}
```

- [ ] **Step 4: Export infra modules**

Replace `src-tauri/crates/hmm-infra/src/lib.rs` with:

```rust
mod game_config_repository;
mod game_directory_probe;
mod game_discovery;

use anyhow::Result;
use hmm_ports::AppClock;
use std::time::{SystemTime, UNIX_EPOCH};

pub use game_config_repository::JsonGameConfigRepository;
pub use game_directory_probe::{RealGameDirectoryProbe, RealGameDirectoryProbeFactory};
pub use game_discovery::NoopGameDiscoveryService;

pub struct SystemClock;

impl AppClock for SystemClock {
    fn now_unix_millis(&self) -> Result<u128> {
        Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
    }
}
```

- [ ] **Step 5: Add dependencies**

Modify `src-tauri/crates/hmm-infra/Cargo.toml`:

```toml
[dependencies]
anyhow.workspace = true
hmm-core = { path = "../hmm-core" }
hmm-ports = { path = "../hmm-ports" }
serde.workspace = true
serde_json.workspace = true
```

- [ ] **Step 6: Run infra tests**

Run:

```powershell
cargo test -p hmm-infra
```

Expected:

```text
test result: ok
```

- [ ] **Step 7: Commit infra**

Run:

```powershell
git add src-tauri/crates/hmm-infra
git commit -m "feat: 添加游戏目录配置基础设施"
```

## Task 6: Tauri State, DTOs, and Commands

**Files:**

- Create: `src-tauri/src/dto.rs`
- Create: `src-tauri/src/game_setup_commands.rs`
- Create: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `cargo test -p hmm-tauri`

- [ ] **Step 1: Add DTO mapping**

Create `src-tauri/src/dto.rs`:

```rust
use hmm_app::GameSetupServiceError;
use hmm_core::{
    GameDirectoryEvidence, GameDirectoryStatus, GameDirectoryValidation, GameInstance, GameSetupErrorCode, GameSetupStatus,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandErrorDto {
    pub code: String,
    pub message: String,
}

impl CommandErrorDto {
    pub fn from_service_error(error: GameSetupServiceError) -> Self {
        let code = match error.error_code() {
            GameSetupErrorCode::UnsupportedGame => "unsupported_game",
            GameSetupErrorCode::DirectoryNotFound => "directory_not_found",
            GameSetupErrorCode::MissingExecutable => "missing_executable",
            GameSetupErrorCode::StorageFailed => "storage_failed",
            GameSetupErrorCode::StorageCorrupted => "storage_corrupted",
            GameSetupErrorCode::ScanNotImplemented => "scan_not_implemented",
            GameSetupErrorCode::Unknown => "unknown",
        }
        .to_owned();

        Self {
            code,
            message: error.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSetupStatusDto {
    pub game_id: String,
    pub kind: String,
    pub display_name: Option<String>,
    pub path_label: Option<String>,
    pub error_code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDirectoryValidationDto {
    pub game_id: String,
    pub is_valid: bool,
    pub confidence: u8,
    pub evidence: Vec<GameDirectoryEvidenceDto>,
    pub errors: Vec<String>,
    pub path_label: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDirectoryEvidenceDto {
    pub kind: String,
    pub label: String,
}

pub fn status_to_dto(status: GameSetupStatus) -> GameSetupStatusDto {
    let kind = match status.status {
        GameDirectoryStatus::NotConfigured => "not_configured",
        GameDirectoryStatus::Invalid => "invalid",
        GameDirectoryStatus::Configured => "configured",
    }
    .to_owned();

    let (display_name, path_label) = status
        .instance
        .map(instance_to_display_parts)
        .unwrap_or((None, None));

    GameSetupStatusDto {
        game_id: status.game_id.as_str().to_owned(),
        kind,
        display_name,
        path_label,
        error_code: status.error_code.map(error_code_to_string),
        message: status.message,
    }
}

pub fn validation_to_dto(validation: GameDirectoryValidation) -> GameDirectoryValidationDto {
    GameDirectoryValidationDto {
        game_id: validation.game_id.as_str().to_owned(),
        is_valid: validation.is_valid,
        confidence: validation.confidence,
        evidence: validation.evidence.into_iter().map(evidence_to_dto).collect(),
        errors: validation.errors.into_iter().map(error_code_to_string).collect(),
        path_label: path_label_from_path(&validation.directory),
    }
}

fn instance_to_display_parts(instance: GameInstance) -> (Option<String>, Option<String>) {
    (
        Some(instance.display_name),
        Some(path_label_from_path(&instance.root_dir)),
    )
}

fn evidence_to_dto(evidence: GameDirectoryEvidence) -> GameDirectoryEvidenceDto {
    GameDirectoryEvidenceDto {
        kind: format!("{:?}", evidence.kind).to_ascii_lowercase(),
        label: evidence.label,
    }
}

fn error_code_to_string(error: GameSetupErrorCode) -> String {
    match error {
        GameSetupErrorCode::UnsupportedGame => "unsupported_game",
        GameSetupErrorCode::DirectoryNotFound => "directory_not_found",
        GameSetupErrorCode::MissingExecutable => "missing_executable",
        GameSetupErrorCode::StorageFailed => "storage_failed",
        GameSetupErrorCode::StorageCorrupted => "storage_corrupted",
        GameSetupErrorCode::ScanNotImplemented => "scan_not_implemented",
        GameSetupErrorCode::Unknown => "unknown",
    }
    .to_owned()
}

fn path_label_from_path(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!(".../{name}"))
        .unwrap_or_else(|| ".../selected-directory".to_owned())
}
```

- [ ] **Step 2: Add state composition**

Create `src-tauri/src/state.rs`:

```rust
use hmm_app::GameSetupService;
use hmm_games_mhw::MonsterHunterWorldAdapter;
use hmm_infra::{JsonGameConfigRepository, NoopGameDiscoveryService, RealGameDirectoryProbeFactory, SystemClock};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

pub struct AppState {
    pub game_setup: Mutex<GameSetupService>,
}

impl AppState {
    pub fn new(app_handle: &AppHandle) -> Result<Self, String> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|error| format!("failed to resolve app data dir: {error}"))?;
        let config_path = app_data_dir.join("config").join("games.json");

        Ok(Self {
            game_setup: Mutex::new(GameSetupService::new(
                vec![Arc::new(MonsterHunterWorldAdapter)],
                Arc::new(JsonGameConfigRepository::new(config_path)),
                Arc::new(RealGameDirectoryProbeFactory),
                Arc::new(NoopGameDiscoveryService),
                Arc::new(SystemClock),
            )),
        })
    }
}
```

- [ ] **Step 3: Add commands**

Create `src-tauri/src/game_setup_commands.rs`:

```rust
use crate::dto::{status_to_dto, validation_to_dto, CommandErrorDto, GameDirectoryValidationDto, GameSetupStatusDto};
use crate::state::AppState;
use hmm_core::GameId;
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub fn get_game_setup_status(game_id: String, state: State<'_, AppState>) -> Result<GameSetupStatusDto, CommandErrorDto> {
    let game_id = parse_game_id(game_id)?;
    let service = state.game_setup.lock().map_err(|_| CommandErrorDto {
        code: "unknown".to_owned(),
        message: "game setup state lock failed".to_owned(),
    })?;

    service
        .get_status(game_id)
        .map(status_to_dto)
        .map_err(CommandErrorDto::from_service_error)
}

#[tauri::command]
pub fn validate_game_directory(
    game_id: String,
    directory: String,
    state: State<'_, AppState>,
) -> Result<GameDirectoryValidationDto, CommandErrorDto> {
    let game_id = parse_game_id(game_id)?;
    let directory = parse_directory(directory)?;
    let service = state.game_setup.lock().map_err(|_| CommandErrorDto {
        code: "unknown".to_owned(),
        message: "game setup state lock failed".to_owned(),
    })?;

    service
        .validate_directory(game_id, directory)
        .map(validation_to_dto)
        .map_err(CommandErrorDto::from_service_error)
}

#[tauri::command]
pub fn save_game_directory(
    game_id: String,
    directory: String,
    state: State<'_, AppState>,
) -> Result<GameSetupStatusDto, CommandErrorDto> {
    let game_id = parse_game_id(game_id)?;
    let directory = parse_directory(directory)?;
    let service = state.game_setup.lock().map_err(|_| CommandErrorDto {
        code: "unknown".to_owned(),
        message: "game setup state lock failed".to_owned(),
    })?;

    service
        .save_game_directory(game_id, directory)
        .map(status_to_dto)
        .map_err(CommandErrorDto::from_service_error)
}

#[tauri::command]
pub fn scan_game_candidates(game_id: String, state: State<'_, AppState>) -> Result<(), CommandErrorDto> {
    let game_id = parse_game_id(game_id)?;
    let service = state.game_setup.lock().map_err(|_| CommandErrorDto {
        code: "unknown".to_owned(),
        message: "game setup state lock failed".to_owned(),
    })?;

    service
        .scan_candidates(game_id)
        .map_err(CommandErrorDto::from_service_error)
}

fn parse_game_id(value: String) -> Result<GameId, CommandErrorDto> {
    GameId::parse(value).map_err(|error| CommandErrorDto {
        code: "unsupported_game".to_owned(),
        message: error.to_string(),
    })
}

fn parse_directory(value: String) -> Result<PathBuf, CommandErrorDto> {
    if value.trim().is_empty() {
        return Err(CommandErrorDto {
            code: "directory_not_found".to_owned(),
            message: "directory cannot be empty".to_owned(),
        });
    }

    Ok(PathBuf::from(value))
}
```

- [ ] **Step 4: Wire commands into Tauri builder**

Replace `src-tauri/src/lib.rs` with:

```rust
mod dto;
mod game_setup_commands;
mod state;

use game_setup_commands::{
    get_game_setup_status, save_game_directory, scan_game_candidates, validate_game_directory,
};
use state::AppState;

#[tauri::command]
fn app_health() -> &'static str {
    "ok"
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = AppState::new(&app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_health,
            get_game_setup_status,
            validate_game_directory,
            save_game_directory,
            scan_game_candidates
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Helsincy Mod Manager");
}

#[cfg(test)]
mod tests {
    use super::app_health;

    #[test]
    fn app_health_returns_ok() {
        assert_eq!(app_health(), "ok");
    }
}
```

- [ ] **Step 5: Add Tauri dependencies**

Modify `src-tauri/Cargo.toml` dependencies:

```toml
[dependencies]
hmm-app = { path = "crates/hmm-app" }
hmm-core = { path = "crates/hmm-core" }
hmm-games-mhw = { path = "crates/hmm-games-mhw" }
hmm-infra = { path = "crates/hmm-infra" }
serde.workspace = true
serde_json.workspace = true
tauri = { workspace = true, features = [] }
tauri-plugin-dialog = "2"
tracing.workspace = true
```

- [ ] **Step 6: Run Tauri crate tests**

Run:

```powershell
cargo test -p hmm-tauri
```

Expected:

```text
test result: ok
```

- [ ] **Step 7: Commit Tauri command layer**

Run:

```powershell
git add src-tauri/src src-tauri/Cargo.toml Cargo.lock
git commit -m "feat: 添加游戏目录配置 Tauri 命令"
```

## Task 7: Dialog Plugin and Frontend Dependency

**Files:**

- Modify: `package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `src-tauri/capabilities/default.json`
- Test: `cmd /c corepack pnpm install --frozen-lockfile`

- [ ] **Step 1: Add dialog plugin package**

Run:

```powershell
cmd /c corepack pnpm add @tauri-apps/plugin-dialog
```

Expected:

```text
dependencies:
+ @tauri-apps/plugin-dialog
```

- [ ] **Step 2: Allow dialog permission**

Modify `src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default desktop capability for Helsincy Mod Manager.",
  "windows": ["main"],
  "permissions": ["core:default", "dialog:default"]
}
```

- [ ] **Step 3: Verify lockfile is stable**

Run:

```powershell
cmd /c corepack pnpm install --frozen-lockfile
```

Expected:

```text
Lockfile is up to date
```

- [ ] **Step 4: Commit dialog dependency**

Run:

```powershell
git add package.json pnpm-lock.yaml src-tauri/capabilities/default.json
git commit -m "feat: 接入目录选择对话框插件"
```

## Task 8: Frontend Game Setup Feature

**Files:**

- Create: `src/features/game-setup/gameSetupTypes.ts`
- Create: `src/features/game-setup/gameSetupApi.ts`
- Create: `src/features/game-setup/gameSetupViewModel.ts`
- Create: `src/features/game-setup/useGameSetup.ts`
- Create: `src/features/game-setup/GameDirectoryActions.tsx`
- Modify: `src/shared/api/tauri.ts`
- Reference: `docs/superpowers/plans/2026-06-04-game-directory-settings-implementation-frontend-appendix.md`
- Test: `cmd /c corepack pnpm run typecheck`

完整代码片段放在同目录前端附录中，避免单个 Markdown 文件超过项目治理硬线。执行 Task 8 时必须逐步执行附录中的 Step 1-8，不要自行改写 API 形状。

- [ ] **Step 1: Create frontend game setup files from appendix**

Follow `docs/superpowers/plans/2026-06-04-game-directory-settings-implementation-frontend-appendix.md` Step 1-6 exactly.

- [ ] **Step 2: Run frontend typecheck**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected:

```text
No TypeScript errors
```

- [ ] **Step 3: Commit game setup frontend feature**

Run:

```powershell
git add src/features/game-setup src/shared/api/tauri.ts
git commit -m "feat: 添加前端游戏目录配置状态"
```
## Task 9: Dashboard Integration

**Files:**

- Modify: `src/features/dashboard/DashboardPage.tsx`
- Modify: `src/features/dashboard/DashboardHeroCard.tsx`
- Modify: `src/features/dashboard/SetupStatusPanel.tsx`
- Modify: `src/features/dashboard/dashboardData.ts`
- Modify: `src/features/dashboard/Dashboard.css`
- Test: `cmd /c corepack pnpm run typecheck`
- Test: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1`

- [ ] **Step 1: Wire hook into DashboardPage**

Modify `src/features/dashboard/DashboardPage.tsx`:

```tsx
import { useGameSetup } from "../game-setup/useGameSetup";
import { DashboardHeroCard } from "./DashboardHeroCard";
import { DashboardModulePreview } from "./DashboardModulePreview";
import { SetupStatusPanel } from "./SetupStatusPanel";

export function DashboardPage() {
  const gameSetup = useGameSetup("mhw");

  return (
    <>
      <section className="main-workspace" aria-labelledby="workbench-title">
        <header className="main-header">
          <h2 id="workbench-title">工作台</h2>
          <p>首次启动需要先完成游戏目录识别。</p>
        </header>

        <DashboardHeroCard
          status={gameSetup.status}
          isBusy={gameSetup.isBusy}
          actionMessage={gameSetup.actionMessage}
          onDirectorySelected={gameSetup.saveDirectory}
          onScanSteam={gameSetup.scanSteam}
        />
        <DashboardModulePreview />
      </section>

      <SetupStatusPanel status={gameSetup.status} actionMessage={gameSetup.actionMessage} />
    </>
  );
}
```

- [ ] **Step 2: Replace static hero state**

Modify `src/features/dashboard/DashboardHeroCard.tsx`:

```tsx
import { GameDirectoryActions } from "../game-setup/GameDirectoryActions";
import type { GameSetupStatus } from "../game-setup/gameSetupTypes";
import { supportCards } from "./dashboardData";

type DashboardHeroCardProps = {
  status: GameSetupStatus;
  isBusy: boolean;
  actionMessage: string | null;
  onDirectorySelected: (directory: string) => Promise<void>;
  onScanSteam: () => Promise<void>;
};

export function DashboardHeroCard({
  status,
  isBusy,
  actionMessage,
  onDirectorySelected,
  onScanSteam,
}: DashboardHeroCardProps) {
  const copy = heroCopyForStatus(status, actionMessage);

  return (
    <section className="setup-panel" aria-labelledby="setup-title">
      <div className="setup-message">
        <span className={`badge ${copy.badgeTone}`}>
          <span className={`dot ${copy.dotClass}`} aria-hidden="true" />
          {copy.badge}
        </span>
        <h3 id="setup-title">{copy.title}</h3>
        <p>{copy.description}</p>
      </div>

      <GameDirectoryActions isBusy={isBusy} onDirectorySelected={onDirectorySelected} onScanSteam={onScanSteam} />

      <div className="support-grid" aria-label="支持信息">
        {supportCards.map((card) => (
          <article className="support-card group" key={card.label}>
            <div className="support-card-header">
              <card.icon size={16} color={card.iconColor} strokeWidth={2.1} />
              <span>{card.label}</span>
            </div>
            <strong>{card.value}</strong>
          </article>
        ))}
      </div>
    </section>
  );
}

function heroCopyForStatus(status: GameSetupStatus, actionMessage: string | null) {
  if (status.kind === "configured") {
    return {
      badge: "目录已配置",
      badgeTone: "success",
      dotClass: "success-dot",
      title: status.displayName,
      description: `当前目录：${status.pathLabel}`,
    };
  }

  if (status.kind === "validating") {
    return {
      badge: "正在校验",
      badgeTone: "warning",
      dotClass: "warning-dot",
      title: "正在验证游戏目录",
      description: "Helsincy 正在确认所选目录是否包含 MHW:I 可执行文件。",
    };
  }

  if (status.kind === "invalid") {
    return {
      badge: "校验失败",
      badgeTone: "danger",
      dotClass: "danger-dot",
      title: "目录校验未通过",
      description: actionMessage ?? status.message,
    };
  }

  return {
    badge: "目录未配置",
    badgeTone: "warning",
    dotClass: "warning-dot",
    title: "未找到游戏目录",
    description: "需要先识别《怪物猎人：世界 冰原》的安装目录，才能导入和安装 Mod。",
  };
}
```

- [ ] **Step 3: Make setup status panel dynamic**

Modify `src/features/dashboard/SetupStatusPanel.tsx` to accept props and derive status copy:

```tsx
import type { GameSetupStatus } from "../game-setup/gameSetupTypes";
import { setupLogs, setupSteps } from "./dashboardData";

type SetupStatusPanelProps = {
  status: GameSetupStatus;
  actionMessage: string | null;
};

export function SetupStatusPanel({ status, actionMessage }: SetupStatusPanelProps) {
  const copy = statusPanelCopy(status, actionMessage);

  return (
    <aside className="setup-rail" aria-label="首次启动设置状态">
      <header className="rail-header">
        <span>首次启动</span>
        <h2>设置状态</h2>
        <p>Helsincy 需要先完成几项检查，才能启用模组管理。</p>
      </header>

      <section className="rail-card current-state" aria-labelledby="current-state-title">
        <div className="state-title-row">
          <span className={`dot ${copy.dotClass}`} aria-hidden="true" />
          <h3 id="current-state-title">{copy.title}</h3>
        </div>
        <p>{copy.description}</p>
        <span className="soft-badge">{copy.badge}</span>
      </section>

      <section className="rail-section" aria-labelledby="next-step-title">
        <div className="section-title-row">
          <h3 id="next-step-title">下一步</h3>
          <span>{copy.stepLabel}</span>
        </div>
        <div className="step-list">
          {setupSteps.map((step, index) => (
            <StepItem key={step.title} index={index + 1} step={step} isLast={index === setupSteps.length - 1} />
          ))}
        </div>
      </section>

      <section className="rail-section" aria-labelledby="summary-title">
        <h3 id="summary-title">设置摘要</h3>
        <div className="summary-grid">
          <SummaryBox label="状态" value={copy.summaryStatus} />
          <SummaryBox label="风险" value={copy.summaryRisk} />
        </div>
        <article className="summary-note">
          <strong>{copy.noteTitle}</strong>
          <p>{copy.noteBody}</p>
        </article>
      </section>

      <section className="rail-section" aria-labelledby="setup-log-title">
        <h3 id="setup-log-title">设置日志</h3>
        <div className="log-card">
          {setupLogs.map((log) => (
            <p key={`${log.time}-${log.message}`} className={"muted" in log && log.muted ? "is-muted" : ""}>
              <time>{log.time}</time>
              {log.message}
            </p>
          ))}
        </div>
      </section>
    </aside>
  );
}

function statusPanelCopy(status: GameSetupStatus, actionMessage: string | null) {
  if (status.kind === "configured") {
    return {
      dotClass: "success-dot",
      title: "游戏目录已配置",
      description: `当前目录：${status.pathLabel}`,
      badge: "可继续配置",
      stepLabel: "第 2 / 4 步",
      summaryStatus: "已配置",
      summaryRisk: "低：未写入游戏目录",
      noteTitle: "配置已保存",
      noteBody: "当前仅保存目录配置，尚未执行 Mod 安装或游戏目录写入。",
    };
  }

  if (status.kind === "invalid") {
    return {
      dotClass: "danger-dot",
      title: "目录校验失败",
      description: actionMessage ?? status.message,
      badge: "需要重新选择",
      stepLabel: "第 1 / 4 步",
      summaryStatus: "校验失败",
      summaryRisk: "中：目录不可用",
      noteTitle: "检查未通过",
      noteBody: "请选择包含 MonsterHunterWorld.exe 的游戏根目录。",
    };
  }

  if (status.kind === "validating") {
    return {
      dotClass: "warning-dot",
      title: "正在校验目录",
      description: "正在确认所选目录是否可作为 MHW:I 游戏根目录。",
      badge: "校验中",
      stepLabel: "第 1 / 4 步",
      summaryStatus: "校验中",
      summaryRisk: "低：只读检查",
      noteTitle: "校验进行中",
      noteBody: "当前检查不会创建 nativePC，也不会写入游戏目录。",
    };
  }

  return {
    dotClass: "neutral-dot",
    title: "等待选择游戏目录",
    description: "尚未选择游戏目录。请先手动选择 MHW:I 安装目录。",
    badge: "等待主区操作",
    stepLabel: "第 1 / 4 步",
    summaryStatus: "未配置",
    summaryRisk: "低：未开始",
    noteTitle: "检查等待中",
    noteBody: "将在设置过程中检查游戏目录身份和配置存储。",
  };
}
```

Keep existing `StepItem` and `SummaryBox` helper functions below this new copy function.

- [ ] **Step 4: Update setup steps to match first-version scope**

Modify `setupSteps` in `src/features/dashboard/dashboardData.ts`:

```ts
export const setupSteps = [
  {
    title: "选择游戏目录",
    meta: "手动选择 MHW:I 安装根目录。",
    active: true,
  },
  {
    title: "验证游戏目录",
    meta: "确认目录中存在 MonsterHunterWorld.exe。",
  },
  {
    title: "保存配置",
    meta: "在应用数据目录创建游戏配置文件。",
  },
  {
    title: "继续配置模组",
    meta: "目录可用后再启用导入与前置检查。",
  },
] as const;
```

- [ ] **Step 5: Add danger badge styles if missing**

Modify `src/features/dashboard/Dashboard.css` only if it does not already define danger tone:

```css
.badge.danger {
  background: var(--color-danger-alpha-12);
  color: var(--color-danger);
}

.danger-dot {
  background: var(--color-danger);
}
```

If `--color-danger` or `--color-danger-alpha-12` is missing from `src/shared/styles/tokens.css`, add them there using the existing token naming style.

- [ ] **Step 6: Run frontend checks**

Run:

```powershell
cmd /c corepack pnpm run typecheck
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1
```

Expected:

```text
No TypeScript errors
Frontend boundary checks passed
```

- [ ] **Step 7: Commit Dashboard integration**

Run:

```powershell
git add src/features/dashboard src/features/game-setup src/shared/styles/tokens.css
git commit -m "feat: 接入 Dashboard 游戏目录状态"
```

## Task 10: End-to-End Verification

**Files:**

- Verify: whole workspace
- Test: frontend, Rust, unified scripts

- [ ] **Step 1: Check formatting-sensitive diffs**

Run:

```powershell
git diff --check
```

Expected:

```text
```

No output means no whitespace errors.

- [ ] **Step 2: Run frontend checks**

Run:

```powershell
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

Expected:

```text
No TypeScript errors
No ESLint errors
vite build completes successfully
```

- [ ] **Step 3: Run Rust checks**

Run:

```powershell
cargo test --workspace
cargo check --workspace
```

Expected:

```text
test result: ok
Finished dev profile
```

- [ ] **Step 4: Run governance checks**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected:

```text
Verify completed successfully
```

- [ ] **Step 5: Manual app smoke test**

Run:

```powershell
cmd /c corepack pnpm run tauri:dev
```

Manual checks:

- Dashboard starts in “目录未配置” when no `games.json` exists.
- “自动扫描 Steam” shows the scan-not-enabled message.
- “手动选择游戏目录” opens a directory picker.
- Selecting a temp directory without `MonsterHunterWorld.exe` shows “缺少 MonsterHunterWorld.exe” related copy.
- Selecting a temp directory containing an empty `MonsterHunterWorld.exe` saves configuration and moves Dashboard to “目录已配置”.
- The smoke test uses a temp fake directory only, not a real MHW:I install directory.

- [ ] **Step 6: Handle verification fixes without a catch-all commit**

If Task 10 exposes a failure, return to the task that owns the failing file and repeat that task's test and commit step with exact paths from that task. Do not create a catch-all verification commit from this section.

Do not stage `.planning/`, `.plan-attestation`, `__pycache__/`, `*.pyc`, generated app data, fake game directories, or local logs.

## PR Handoff

- [ ] **Step 1: Confirm branch status**

Run:

```powershell
git status --short --branch
```

Expected during the implementation PR:

```text
## codex/game-directory-settings-implementation
```

Working tree must be clean.

- [ ] **Step 2: Push branch**

Run:

```powershell
git push -u origin codex/game-directory-settings-implementation
```

- [ ] **Step 3: Create PR**

Run:

```powershell
gh pr create --base main --head codex/game-directory-settings-implementation --title "[codex] 实现 MHW 游戏目录配置闭环" --body "## 改动\n- 添加 MHW:I 游戏目录配置领域模型、接口、应用用例和基础设施\n- 添加 Tauri 命令与目录选择对话框能力\n- 将 Dashboard 首次启动状态接入真实配置状态\n\n## 验证\n- cargo test --workspace\n- cargo check --workspace\n- cmd /c corepack pnpm run typecheck\n- cmd /c corepack pnpm run lint\n- cmd /c corepack pnpm run build\n- powershell -NoProfile -ExecutionPolicy Bypass -File .\\scripts\\check-frontend-boundaries.ps1\n- powershell -NoProfile -ExecutionPolicy Bypass -File .\\scripts\\verify.ps1\n\n## 风险\n- 自动 Steam 扫描本轮明确返回 scan_not_implemented\n- 本轮不写入真实游戏目录，不读取真实存档，不安装 Mod\n- 手动 smoke test 使用临时假目录验证"
```

## Self-Review Checklist

- [ ] `MonsterHunterWorld.exe` 只出现在 `hmm-games-mhw` 和用户可见错误文案，不出现在通用核心逻辑里。
- [ ] React 组件没有直接调用 `invoke`。
- [ ] Dashboard 没有读取真实文件系统路径规则。
- [ ] JSON repository 写入 app data 的 `config/games.json`，不写仓库目录和游戏目录。
- [ ] 损坏 JSON 返回 `storage_corrupted`，不会静默覆盖。
- [ ] 自动扫描返回 `scan_not_implemented`，不会假装已经扫描 Steam。
- [ ] 测试只使用 temp/fake 目录。
- [ ] 最终回复准确列出已执行和未执行的验证。
