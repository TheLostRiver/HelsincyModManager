# Mod Installation MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现第一条可测试的 Mod 安装链路：从已分析的文件提供者生成 `InstallPlan`，通过安全物理复制后端写入临时游戏目录，覆盖前备份，完成后写入 `InstallManifest`，失败时可回滚。

**Architecture:** `hmm-core` 定义纯领域模型和路径规则；`hmm-ports` 定义安装后端、manifest 仓储、文件状态探测和事件端口；`hmm-app` 负责编排计划构建、提交、卸载和恢复；`hmm-infra` 实现真实文件系统复制、备份、hash 与 JSON manifest 存储；Tauri command 只做 DTO 转换和用例转发。MVP 只实现安全物理复制后端，虚拟映射作为后续后端接入同一套 `InstallPlan` / `InstallManifest`，不在本轮实现。

**Tech Stack:** Rust workspace、Tauri 2、React 19、TypeScript、serde、serde_json、thiserror、fs2、临时目录测试、PowerShell `scripts/verify.ps1`。

---

## Scope

本计划实现：

- 安装相关领域模型：`ModId`、`ProfileId`、`PackageFileId`、`InstallTargetPath`、`ModFileProvider`、`FileLayerStack`、`InstallPlan`、`InstallAction`、`InstallManifest`。
- 目标路径安全校验：拒绝空路径、绝对路径、路径穿越、Windows drive prefix、adapter 未允许的目标根。
- 文件层栈解析：同一目标路径多个 provider 时，显式优先级可解决；没有显式优先级时输出阻断冲突。
- 复制后端 MVP：创建目录、覆盖前备份、复制文件、删除本工具安装的文件、写 manifest。
- 基于 manifest 卸载：恢复备份或删除本工具新增文件。
- 失败回滚：复制失败或 manifest 写入失败时尽最大努力恢复已变更文件。
- 临时目录测试：不读写真实游戏目录、真实存档或真实第三方 Mod 包。

本计划不实现：

- 压缩包真实解压器和预览图提取。
- Profile UI 和批量启用/禁用 UI。
- 替换目标 retarget staging。
- 虚拟映射、symlink、junction 或文件系统挂载。
- SQLite manifest 存储；MVP 使用 JSON 仓储，后续可替换。
- 真实游戏目录手动 smoke 写入；本轮只用临时目录。

## Target File Structure

```text
src-tauri/
  crates/
    hmm-core/src/
      game.rs
      install.rs
      lib.rs
    hmm-ports/src/
      game_setup.rs
      install.rs
      lib.rs
    hmm-app/src/
      game_setup.rs
      install.rs
      lib.rs
    hmm-infra/src/
      game_config_repository.rs
      game_directory_probe.rs
      game_discovery.rs
      install_backend.rs
      install_manifest_repository.rs
      lib.rs
    hmm-games-mhw/src/
      lib.rs
  src/
    dto.rs
    install_commands.rs
    lib.rs
    state.rs

src/
  features/
    mods/
      modInstallApi.ts
      modInstallTypes.ts
```

职责锁定：

- `hmm-core/src/install.rs`：领域类型、路径校验、冲突、计划和 manifest，不访问真实文件系统。
- `hmm-ports/src/install.rs`：安装后端和仓储 trait，不包含 JSON、Tauri 或平台实现。
- `hmm-app/src/install.rs`：计划构建、提交、卸载、恢复编排，只依赖 traits。
- `hmm-infra/src/install_backend.rs`：真实文件系统复制、备份、hash、回滚。
- `hmm-infra/src/install_manifest_repository.rs`：JSON manifest 原子写入和读取。
- `hmm-games-mhw/src/lib.rs`：声明 MHW:I 允许的安装目标根，例如 `nativePC` 和 adapter 允许的根目录文件规则。
- `src-tauri/src/install_commands.rs`：仅暴露计划预览、提交、卸载、恢复命令，不直接拼路径或写文件。
- `src/features/mods/*`：仅定义 typed API；完整 UI 放到后续 Profile / Mod 库任务。

## Task 0: Preflight

**Files:**

- Read: `AGENTS.md`
- Read: `docs/mod_installation_strategy.md`
- Read: `docs/ARCHITECTURE.md`
- Read: `docs/TESTING.md`
- Read: `SECURITY.md`
- Read: `CONTRIBUTING.md`
- Read: `src-tauri/crates/hmm-core/src/lib.rs`
- Read: `src-tauri/crates/hmm-ports/src/lib.rs`
- Read: `src-tauri/crates/hmm-app/src/lib.rs`
- Read: `src-tauri/crates/hmm-infra/src/lib.rs`

- [ ] **Step 1: Confirm branch and unrelated changes**

Run:

```powershell
git status --short --branch --untracked-files=all
```

Expected:

```text
## codex/mod-installation-mvp
```

If unrelated files are present, record them in the task notes and do not modify them. Do not stage `.planning/`, `.plan-attestation`, generated app data, fake game directories, local logs, backup directories, real Mod packages or real save files.

- [ ] **Step 2: Confirm baseline tests**

Run:

```powershell
cargo test --workspace
cargo check --workspace
cmd /c corepack pnpm run typecheck
```

Expected:

```text
test result: ok
Finished dev profile
No TypeScript errors
```

- [ ] **Step 3: Confirm MVP safety boundary**

Before writing code, write this note into the task progress file:

```text
MVP install implementation uses temp directories in tests only. It does not write to a real game directory, does not read real saves, and does not consume real third-party Mod packages.
```

## Task 1: Core Install Identifiers and Safe Target Paths

**Files:**

- Create: `src-tauri/crates/hmm-core/src/install.rs`
- Modify: `src-tauri/crates/hmm-core/src/lib.rs`
- Test: `cargo test -p hmm-core install_target_path`

- [ ] **Step 1: Create failing tests for target path validation**

Create `src-tauri/crates/hmm-core/src/install.rs` with tests first:

```rust
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallTargetPathError {
    #[error("target path cannot be empty")]
    Empty,
    #[error("target path must be relative")]
    Absolute,
    #[error("target path cannot contain parent directory segments")]
    ParentSegment,
    #[error("target path root is not allowed: {0}")]
    DisallowedRoot(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots() -> Vec<&'static str> {
        vec!["nativePC", "MonsterLoader.dll"]
    }

    #[test]
    fn install_target_path_accepts_allowed_native_pc_path() {
        let path = InstallTargetPath::parse("nativePC/chunk/item/file.mod3", &roots())
            .expect("path should be valid");

        assert_eq!(path.as_str(), "nativePC/chunk/item/file.mod3");
    }

    #[test]
    fn install_target_path_accepts_allowed_root_file() {
        let path = InstallTargetPath::parse("MonsterLoader.dll", &roots())
            .expect("root file should be valid");

        assert_eq!(path.as_str(), "MonsterLoader.dll");
    }

    #[test]
    fn install_target_path_rejects_absolute_path() {
        let error = InstallTargetPath::parse("C:/Games/file.mod3", &roots())
            .expect_err("absolute path should fail");

        assert_eq!(error, InstallTargetPathError::Absolute);
    }

    #[test]
    fn install_target_path_rejects_parent_segments() {
        let error = InstallTargetPath::parse("nativePC/../evil.bin", &roots())
            .expect_err("parent segment should fail");

        assert_eq!(error, InstallTargetPathError::ParentSegment);
    }

    #[test]
    fn install_target_path_rejects_disallowed_root() {
        let error = InstallTargetPath::parse("unknown/file.bin", &roots())
            .expect_err("unknown root should fail");

        assert_eq!(
            error,
            InstallTargetPathError::DisallowedRoot("unknown".to_owned())
        );
    }
}
```

- [ ] **Step 2: Implement identifiers and target path**

Above the test module in `install.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModId(String);

impl ModId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileId(String);

impl ProfileId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackageFileId(String);

impl PackageFileId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstallTargetPath(String);

impl InstallTargetPath {
    pub fn parse(
        value: impl AsRef<str>,
        allowed_roots: &[&str],
    ) -> Result<Self, InstallTargetPathError> {
        let value = value.as_ref().replace('\\', "/");
        let trimmed = value.trim_matches('/');

        if trimmed.trim().is_empty() {
            return Err(InstallTargetPathError::Empty);
        }

        let path = Path::new(trimmed);
        let mut parts = Vec::new();

        for component in path.components() {
            match component {
                Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
                Component::CurDir => {}
                Component::ParentDir => return Err(InstallTargetPathError::ParentSegment),
                Component::RootDir | Component::Prefix(_) => {
                    return Err(InstallTargetPathError::Absolute);
                }
            }
        }

        let root = parts.first().cloned().ok_or(InstallTargetPathError::Empty)?;
        let allowed = allowed_roots.iter().any(|allowed| {
            if allowed.contains('/') || allowed.contains('\\') {
                normalize_root(allowed) == normalize_root(trimmed)
            } else {
                normalize_root(allowed) == normalize_root(&root)
            }
        });

        if !allowed {
            return Err(InstallTargetPathError::DisallowedRoot(root));
        }

        Ok(Self(parts.join("/")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_path_buf(&self) -> PathBuf {
        self.0.split('/').collect()
    }
}

fn normalize_root(value: &str) -> String {
    if cfg!(any(target_os = "windows", target_os = "macos")) {
        value.replace('\\', "/").to_lowercase()
    } else {
        value.replace('\\', "/")
    }
}
```

- [ ] **Step 3: Export install module**

Modify `src-tauri/crates/hmm-core/src/lib.rs`:

```rust
mod game;
mod install;

pub use game::{
    GameDirectoryEvidence, GameDirectoryEvidenceKind, GameDirectoryStatus, GameDirectoryValidation,
    GameId, GameIdError, GameInstance, GameSetupErrorCode, GameSetupStatus, MHW_GAME_ID,
};

pub use install::{
    InstallTargetPath, InstallTargetPathError, ModId, PackageFileId, ProfileId,
};
```

- [ ] **Step 4: Run core path tests**

Run:

```powershell
cargo test -p hmm-core install_target_path
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit target path model**

Run:

```powershell
git add src-tauri/crates/hmm-core/src/install.rs src-tauri/crates/hmm-core/src/lib.rs
git commit -m "feat: 添加安装目标路径模型"
```

## Task 2: Core Install Plan, Manifest and File Layer Stack

**Files:**

- Modify: `src-tauri/crates/hmm-core/src/install.rs`
- Modify: `src-tauri/crates/hmm-core/src/lib.rs`
- Test: `cargo test -p hmm-core install_plan`
- Test: `cargo test -p hmm-core file_layer_stack`

- [ ] **Step 1: Add file provider, conflict and plan tests**

Append these tests to `install.rs`:

```rust
#[cfg(test)]
mod install_plan_tests {
    use super::*;
    use crate::GameId;

    fn target(value: &str) -> InstallTargetPath {
        InstallTargetPath::parse(value, &["nativePC"]).expect("target")
    }

    fn provider(mod_id: &str, target_path: InstallTargetPath, priority: i32) -> ModFileProvider {
        ModFileProvider {
            mod_id: ModId::new(mod_id),
            package_file_id: PackageFileId::new(format!("{mod_id}-file")),
            source_ref: SourceRef::new(format!("cache/{mod_id}/file.bin")),
            source_hash: FileHash::new("sha256", format!("{mod_id}-hash")),
            target_path,
            priority,
            install_kind: InstallKind::Copy,
            replacement_binding_id: None,
            generated_from: None,
        }
    }

    #[test]
    fn file_layer_stack_uses_highest_priority_provider() {
        let target_path = target("nativePC/common/file.bin");
        let low = provider("low", target_path.clone(), 10);
        let high = provider("high", target_path.clone(), 20);

        let stack = FileLayerStack::new(GameId::mhw(), ProfileId::new("default"), target_path)
            .with_provider(low)
            .with_provider(high);

        assert_eq!(stack.active_provider().expect("active").mod_id.as_str(), "high");
    }

    #[test]
    fn file_layer_stack_reports_tie_conflict() {
        let target_path = target("nativePC/common/file.bin");
        let left = provider("left", target_path.clone(), 10);
        let right = provider("right", target_path.clone(), 10);

        let stack = FileLayerStack::new(GameId::mhw(), ProfileId::new("default"), target_path)
            .with_provider(left)
            .with_provider(right);

        assert!(stack.active_provider().is_none());
        assert_eq!(stack.conflict().expect("conflict").providers.len(), 2);
    }

    #[test]
    fn install_plan_blocks_when_conflict_exists() {
        let conflict = InstallConflict {
            target_path: target("nativePC/common/file.bin"),
            providers: vec![ModId::new("a"), ModId::new("b")],
            reason: ConflictReason::PriorityTie,
        };

        let plan = InstallPlan::new(
            InstallPlanId::new("plan-1"),
            GameId::mhw(),
            "mhw-default".to_owned(),
            ProfileId::new("default"),
        )
        .with_conflict(conflict);

        assert!(!plan.is_committable());
    }
}
```

- [ ] **Step 2: Implement install domain types**

Add these types above the tests in `install.rs`:

```rust
use crate::GameId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstallPlanId(String);

impl InstallPlanId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstallManifestId(String);

impl InstallManifestId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef(String);

impl SourceRef {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileHash {
    pub algorithm: String,
    pub value: String,
}

impl FileHash {
    pub fn new(algorithm: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            algorithm: algorithm.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallKind {
    Copy,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModFileProvider {
    pub mod_id: ModId,
    pub package_file_id: PackageFileId,
    pub source_ref: SourceRef,
    pub source_hash: FileHash,
    pub target_path: InstallTargetPath,
    pub priority: i32,
    pub install_kind: InstallKind,
    pub replacement_binding_id: Option<String>,
    pub generated_from: Option<PackageFileId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictReason {
    PriorityTie,
    MissingRequiredDependency,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallConflict {
    pub target_path: InstallTargetPath,
    pub providers: Vec<ModId>,
    pub reason: ConflictReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileLayerStack {
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub target_path: InstallTargetPath,
    pub providers: Vec<ModFileProvider>,
}

impl FileLayerStack {
    pub fn new(game_id: GameId, profile_id: ProfileId, target_path: InstallTargetPath) -> Self {
        Self {
            game_id,
            profile_id,
            target_path,
            providers: Vec::new(),
        }
    }

    pub fn with_provider(mut self, provider: ModFileProvider) -> Self {
        self.providers.push(provider);
        self
    }

    pub fn active_provider(&self) -> Option<&ModFileProvider> {
        let max_priority = self.providers.iter().map(|item| item.priority).max()?;
        let mut winners = self
            .providers
            .iter()
            .filter(|item| item.priority == max_priority);

        let first = winners.next()?;
        if winners.next().is_some() {
            None
        } else {
            Some(first)
        }
    }

    pub fn conflict(&self) -> Option<InstallConflict> {
        if self.providers.len() < 2 || self.active_provider().is_some() {
            return None;
        }

        Some(InstallConflict {
            target_path: self.target_path.clone(),
            providers: self
                .providers
                .iter()
                .map(|provider| provider.mod_id.clone())
                .collect(),
            reason: ConflictReason::PriorityTie,
        })
    }
}
```

- [ ] **Step 3: Implement plan, actions and manifest types**

Continue in `install.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPlan {
    pub id: InstallPlanId,
    pub game_id: GameId,
    pub game_instance_id: String,
    pub profile_id: ProfileId,
    pub actions: Vec<InstallAction>,
    pub conflicts: Vec<InstallConflict>,
}

impl InstallPlan {
    pub fn new(
        id: InstallPlanId,
        game_id: GameId,
        game_instance_id: String,
        profile_id: ProfileId,
    ) -> Self {
        Self {
            id,
            game_id,
            game_instance_id,
            profile_id,
            actions: Vec::new(),
            conflicts: Vec::new(),
        }
    }

    pub fn with_action(mut self, action: InstallAction) -> Self {
        self.actions.push(action);
        self
    }

    pub fn with_conflict(mut self, conflict: InstallConflict) -> Self {
        self.conflicts.push(conflict);
        self
    }

    pub fn is_committable(&self) -> bool {
        self.conflicts.is_empty() && !self.actions.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallAction {
    ValidateGameInstance,
    CreateDirectory { target_dir: InstallTargetPath },
    BackupExistingFile { target_path: InstallTargetPath },
    CopyFile {
        source_ref: SourceRef,
        target_path: InstallTargetPath,
        source_hash: FileHash,
    },
    RemoveFile { target_path: InstallTargetPath },
    WriteManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallBackendKind {
    Copy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallManifestStatus {
    Planned,
    Committing,
    Completed,
    RollbackRequired,
    RolledBack,
    RepairRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledFile {
    pub target_path: InstallTargetPath,
    pub source_ref: SourceRef,
    pub source_hash: FileHash,
    pub installed_hash: FileHash,
    pub install_kind: InstallKind,
    pub previous_state: PreviousFileState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviousFileState {
    Missing,
    BackedUp { backup_ref: String, hash: FileHash },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupRecord {
    pub target_path: InstallTargetPath,
    pub backup_ref: String,
    pub original_hash: FileHash,
    pub original_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallManifest {
    pub id: InstallManifestId,
    pub plan_id: InstallPlanId,
    pub game_id: GameId,
    pub game_instance_id: String,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub backend: InstallBackendKind,
    pub status: InstallManifestStatus,
    pub files: Vec<InstalledFile>,
    pub backups: Vec<BackupRecord>,
}
```

- [ ] **Step 4: Export install types**

Modify `src-tauri/crates/hmm-core/src/lib.rs` export list:

```rust
pub use install::{
    BackupRecord, ConflictReason, FileHash, FileLayerStack, InstallAction, InstallBackendKind,
    InstallConflict, InstallKind, InstallManifest, InstallManifestId, InstallManifestStatus,
    InstallPlan, InstallPlanId, InstallTargetPath, InstallTargetPathError, InstalledFile, ModFileProvider,
    ModId, PackageFileId, PreviousFileState, ProfileId, SourceRef,
};
```

- [ ] **Step 5: Run core install tests**

Run:

```powershell
cargo test -p hmm-core install_plan
cargo test -p hmm-core file_layer_stack
cargo test -p hmm-core install_target_path
```

Expected:

```text
test result: ok
```

- [ ] **Step 6: Commit install domain**

Run:

```powershell
git add src-tauri/crates/hmm-core/src/install.rs src-tauri/crates/hmm-core/src/lib.rs
git commit -m "feat: 添加安装计划和清单领域模型"
```

## Task 3: MHW Adapter Install Target Rules

**Files:**

- Modify: `src-tauri/crates/hmm-ports/src/game_setup.rs`
- Modify: `src-tauri/crates/hmm-games-mhw/src/lib.rs`
- Test: `cargo test -p hmm-games-mhw install_target_roots`

- [ ] **Step 1: Extend GameAdapter with target roots**

In `src-tauri/crates/hmm-ports/src/game_setup.rs`, add a default method:

```rust
pub trait GameAdapter: Send + Sync {
    fn game_id(&self) -> GameId;
    fn display_name(&self) -> &'static str;
    fn validate_directory(&self, probe: &dyn GameDirectoryProbe) -> GameDirectoryValidation;

    fn steam_app_id(&self) -> Option<u32> {
        None
    }

    fn install_target_roots(&self) -> Vec<&'static str> {
        Vec::new()
    }
}
```

- [ ] **Step 2: Implement MHW target roots**

In `src-tauri/crates/hmm-games-mhw/src/lib.rs`, implement:

```rust
fn install_target_roots(&self) -> Vec<&'static str> {
    vec!["nativePC", "MonsterLoader.dll", "dinput8.dll"]
}
```

Keep game-specific file names in the adapter crate. Do not move them to `hmm-core`, `hmm-app` or the frontend.

- [ ] **Step 3: Add adapter tests**

Add tests:

```rust
#[test]
fn install_target_roots_include_native_pc() {
    let adapter = MonsterHunterWorldAdapter;
    assert!(adapter.install_target_roots().contains(&"nativePC"));
}

#[test]
fn install_target_roots_allow_known_root_loader_files() {
    let adapter = MonsterHunterWorldAdapter;
    let roots = adapter.install_target_roots();

    assert!(roots.contains(&"MonsterLoader.dll"));
    assert!(roots.contains(&"dinput8.dll"));
}
```

- [ ] **Step 4: Run adapter tests**

Run:

```powershell
cargo test -p hmm-games-mhw install_target_roots
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit adapter rules**

Run:

```powershell
git add src-tauri/crates/hmm-ports/src/game_setup.rs src-tauri/crates/hmm-games-mhw/src/lib.rs
git commit -m "feat: 声明安装目标根规则"
```

## Task 4: Install Ports

**Files:**

- Create: `src-tauri/crates/hmm-ports/src/install.rs`
- Modify: `src-tauri/crates/hmm-ports/src/lib.rs`
- Test: `cargo test -p hmm-ports`

- [ ] **Step 1: Define install port errors and traits**

Create `src-tauri/crates/hmm-ports/src/install.rs`:

```rust
use hmm_core::{InstallManifest, InstallManifestId, InstallPlan, SourceRef};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstallBackendError {
    #[error("install plan has blocking conflicts")]
    PlanBlocked,
    #[error("source file missing: {0}")]
    SourceMissing(String),
    #[error("game directory invalid: {0}")]
    GameDirectoryInvalid(String),
    #[error("file operation failed: {0}")]
    FileOperationFailed(String),
    #[error("rollback failed: {0}")]
    RollbackFailed(String),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ManifestRepositoryError {
    #[error("manifest storage corrupted")]
    StorageCorrupted,
    #[error("manifest storage failed: {0}")]
    StorageFailed(String),
}

pub type ManifestRepositoryResult<T> = Result<T, ManifestRepositoryError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallCommitRequest {
    pub game_root: PathBuf,
    pub source_root: PathBuf,
    pub backup_root: PathBuf,
    pub plan: InstallPlan,
    pub manifest: InstallManifest,
}

pub trait InstallBackend: Send + Sync {
    fn commit(&self, request: InstallCommitRequest) -> Result<InstallManifest, InstallBackendError>;
    fn uninstall(&self, game_root: &Path, manifest: &InstallManifest) -> Result<InstallManifest, InstallBackendError>;
}

pub trait InstallManifestRepository: Send + Sync {
    fn save_manifest(&self, manifest: &InstallManifest) -> ManifestRepositoryResult<()>;
    fn load_manifest(&self, id: &InstallManifestId) -> ManifestRepositoryResult<Option<InstallManifest>>;
    fn list_manifests(&self) -> ManifestRepositoryResult<Vec<InstallManifest>>;
}

pub trait SourceFileResolver: Send + Sync {
    fn resolve_source(&self, source_root: &Path, source_ref: &SourceRef) -> PathBuf;
}

pub trait InstallEventSink: Send + Sync {
    fn emit_install_event(&self, task_id: &str, message: &str);
}
```

- [ ] **Step 2: Export install ports**

Modify `src-tauri/crates/hmm-ports/src/lib.rs`:

```rust
mod game_setup;
mod install;

use anyhow::Result;

pub use game_setup::{
    GameAdapter, GameCandidate, GameCandidateSource, GameConfigRepository,
    GameConfigRepositoryError, GameConfigRepositoryResult, GameDirectoryProbe,
    GameDirectoryProbeFactory, GameDiscoveryError, GameDiscoveryRequest, GameDiscoveryService,
};
pub use install::{
    InstallBackend, InstallBackendError, InstallCommitRequest, InstallEventSink,
    InstallManifestRepository, ManifestRepositoryError, ManifestRepositoryResult, SourceFileResolver,
};

pub trait AppClock: Send + Sync {
    fn now_unix_millis(&self) -> Result<u128>;
}
```

- [ ] **Step 3: Run ports tests**

Run:

```powershell
cargo test -p hmm-ports
```

Expected:

```text
test result: ok
```

- [ ] **Step 4: Commit install ports**

Run:

```powershell
git add src-tauri/crates/hmm-ports/src/install.rs src-tauri/crates/hmm-ports/src/lib.rs
git commit -m "feat: 添加安装接口层"
```

## Task 5: App Build Install Plan Use Case

**Files:**

- Create: `src-tauri/crates/hmm-app/src/install.rs`
- Modify: `src-tauri/crates/hmm-app/src/lib.rs`
- Test: `cargo test -p hmm-app build_install_plan`

- [ ] **Step 1: Add planning request and tests**

Create `src-tauri/crates/hmm-app/src/install.rs` with tests:

```rust
use hmm_core::*;
use hmm_ports::GameAdapter;
use std::sync::Arc;
use thiserror::Error;

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{GameDirectoryValidation, GameId};
    use hmm_ports::GameDirectoryProbe;
    use std::path::Path;

    struct FakeAdapter;

    impl GameAdapter for FakeAdapter {
        fn game_id(&self) -> GameId {
            GameId::mhw()
        }

        fn display_name(&self) -> &'static str {
            "Fake Game"
        }

        fn validate_directory(&self, probe: &dyn GameDirectoryProbe) -> GameDirectoryValidation {
            GameDirectoryValidation::new(self.game_id(), probe.root_dir().to_path_buf())
        }

        fn install_target_roots(&self) -> Vec<&'static str> {
            vec!["nativePC"]
        }
    }

    fn provider(mod_id: &str, target: &str, priority: i32) -> ModFileProvider {
        ModFileProvider {
            mod_id: ModId::new(mod_id),
            package_file_id: PackageFileId::new(format!("{mod_id}-file")),
            source_ref: SourceRef::new(format!("{mod_id}/file.bin")),
            source_hash: FileHash::new("sha256", format!("{mod_id}-hash")),
            target_path: InstallTargetPath::parse(target, &["nativePC"]).expect("target"),
            priority,
            install_kind: InstallKind::Copy,
            replacement_binding_id: None,
            generated_from: None,
        }
    }

    #[test]
    fn build_install_plan_creates_copy_actions_for_active_provider() {
        let service = InstallPlanningService::new(vec![Arc::new(FakeAdapter)]);
        let request = BuildInstallPlanRequest {
            game_id: GameId::mhw(),
            game_instance_id: "mhw-default".to_owned(),
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("visible-mod"),
            providers: vec![provider("visible-mod", "nativePC/common/file.bin", 10)],
        };

        let plan = service.build_install_plan(request).expect("plan");

        assert!(plan.is_committable());
        assert!(plan.actions.iter().any(|action| matches!(action, InstallAction::CopyFile { .. })));
        assert!(plan.conflicts.is_empty());
    }

    #[test]
    fn build_install_plan_blocks_priority_tie() {
        let service = InstallPlanningService::new(vec![Arc::new(FakeAdapter)]);
        let request = BuildInstallPlanRequest {
            game_id: GameId::mhw(),
            game_instance_id: "mhw-default".to_owned(),
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("left"),
            providers: vec![
                provider("left", "nativePC/common/file.bin", 10),
                provider("right", "nativePC/common/file.bin", 10),
            ],
        };

        let plan = service.build_install_plan(request).expect("plan");

        assert!(!plan.is_committable());
        assert_eq!(plan.conflicts.len(), 1);
    }
}
```

- [ ] **Step 2: Implement planning service**

Above tests in `install.rs`, add:

```rust
#[derive(Debug, Error)]
pub enum InstallPlanningError {
    #[error("unsupported game")]
    UnsupportedGame,
}

pub struct BuildInstallPlanRequest {
    pub game_id: GameId,
    pub game_instance_id: String,
    pub profile_id: ProfileId,
    pub mod_id: ModId,
    pub providers: Vec<ModFileProvider>,
}

pub struct InstallPlanningService {
    adapters: Vec<Arc<dyn GameAdapter>>,
}

impl InstallPlanningService {
    pub fn new(adapters: Vec<Arc<dyn GameAdapter>>) -> Self {
        Self { adapters }
    }

    pub fn build_install_plan(
        &self,
        request: BuildInstallPlanRequest,
    ) -> Result<InstallPlan, InstallPlanningError> {
        let adapter = self.require_adapter(&request.game_id)?;
        let allowed_roots = adapter.install_target_roots();
        let mut plan = InstallPlan::new(
            InstallPlanId::new(format!("plan-{}", request.mod_id.as_str())),
            request.game_id.clone(),
            request.game_instance_id,
            request.profile_id.clone(),
        )
        .with_action(InstallAction::ValidateGameInstance);

        let mut stacks = std::collections::BTreeMap::<String, FileLayerStack>::new();

        for provider in request.providers {
            let normalized = provider.target_path.as_str().to_owned();
            let checked_target =
                InstallTargetPath::parse(provider.target_path.as_str(), &allowed_roots)
                    .map_err(|_| InstallPlanningError::UnsupportedGame)?;
            let provider = ModFileProvider {
                target_path: checked_target.clone(),
                ..provider
            };
            stacks
                .entry(normalized)
                .or_insert_with(|| {
                    FileLayerStack::new(
                        request.game_id.clone(),
                        request.profile_id.clone(),
                        checked_target,
                    )
                })
                .providers
                .push(provider);
        }

        for stack in stacks.into_values() {
            if let Some(conflict) = stack.conflict() {
                plan = plan.with_conflict(conflict);
                continue;
            }

            if let Some(active) = stack.active_provider() {
                plan = plan
                    .with_action(InstallAction::BackupExistingFile {
                        target_path: active.target_path.clone(),
                    })
                    .with_action(InstallAction::CopyFile {
                        source_ref: active.source_ref.clone(),
                        target_path: active.target_path.clone(),
                        source_hash: active.source_hash.clone(),
                    });
            }
        }

        Ok(plan.with_action(InstallAction::WriteManifest))
    }

    fn require_adapter(&self, game_id: &GameId) -> Result<Arc<dyn GameAdapter>, InstallPlanningError> {
        self.adapters
            .iter()
            .find(|adapter| adapter.game_id() == *game_id)
            .cloned()
            .ok_or(InstallPlanningError::UnsupportedGame)
    }
}
```

- [ ] **Step 3: Export install app service**

Modify `src-tauri/crates/hmm-app/src/lib.rs`:

```rust
mod game_setup;
mod install;

pub use game_setup::{GameCandidateScan, GameSetupCandidate, GameSetupService, GameSetupServiceError};
pub use install::{BuildInstallPlanRequest, InstallPlanningError, InstallPlanningService};
```

- [ ] **Step 4: Run planning tests**

Run:

```powershell
cargo test -p hmm-app build_install_plan
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit planning service**

Run:

```powershell
git add src-tauri/crates/hmm-app/src/install.rs src-tauri/crates/hmm-app/src/lib.rs
git commit -m "feat: 生成 Mod 安装计划"
```

## Task 6: JSON Manifest Repository

**Files:**

- Create: `src-tauri/crates/hmm-infra/src/install_manifest_repository.rs`
- Modify: `src-tauri/crates/hmm-infra/src/lib.rs`
- Test: `cargo test -p hmm-infra manifest_repository`

- [ ] **Step 1: Add manifest repository tests**

Create `src-tauri/crates/hmm-infra/src/install_manifest_repository.rs` with tests:

```rust
use hmm_core::*;
use hmm_ports::{InstallManifestRepository, ManifestRepositoryError, ManifestRepositoryResult};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::GameId;

    fn temp_manifest_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "hmm-manifest-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    fn manifest(id: &str) -> InstallManifest {
        InstallManifest {
            id: InstallManifestId::new(id),
            plan_id: InstallPlanId::new("plan-1"),
            game_id: GameId::mhw(),
            game_instance_id: "mhw-default".to_owned(),
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            backend: InstallBackendKind::Copy,
            status: InstallManifestStatus::Completed,
            files: Vec::new(),
            backups: Vec::new(),
        }
    }

    #[test]
    fn manifest_repository_saves_and_loads_manifest() {
        let repo = JsonInstallManifestRepository::new(temp_manifest_dir("save"));

        repo.save_manifest(&manifest("manifest-1")).expect("save");
        let loaded = repo
            .load_manifest(&InstallManifestId::new("manifest-1"))
            .expect("load")
            .expect("manifest");

        assert_eq!(loaded.id.as_str(), "manifest-1");
    }

    #[test]
    fn manifest_repository_lists_manifests() {
        let repo = JsonInstallManifestRepository::new(temp_manifest_dir("list"));

        repo.save_manifest(&manifest("manifest-1")).expect("save one");
        repo.save_manifest(&manifest("manifest-2")).expect("save two");

        let list = repo.list_manifests().expect("list");
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn manifest_repository_reports_corruption() {
        let root = temp_manifest_dir("corrupt");
        fs::create_dir_all(&root).expect("create root");
        fs::write(root.join("broken.json"), "{").expect("write broken");
        let repo = JsonInstallManifestRepository::new(root);

        let error = repo.list_manifests().expect_err("broken manifest should fail");

        assert_eq!(error, ManifestRepositoryError::StorageCorrupted);
    }
}
```

- [ ] **Step 2: Implement JSON repository**

Above tests in the same file, add:

```rust
pub struct JsonInstallManifestRepository {
    root_dir: PathBuf,
    write_lock: Mutex<()>,
}

impl JsonInstallManifestRepository {
    pub fn new(root_dir: PathBuf) -> Self {
        Self {
            root_dir,
            write_lock: Mutex::new(()),
        }
    }

    fn manifest_path(&self, id: &InstallManifestId) -> PathBuf {
        self.root_dir.join(format!("{}.json", sanitize_file_name(id.as_str())))
    }

    fn save_file(&self, path: PathBuf, manifest: &InstallManifest) -> ManifestRepositoryResult<()> {
        fs::create_dir_all(&self.root_dir)
            .map_err(|error| ManifestRepositoryError::StorageFailed(error.to_string()))?;
        let serialized = serde_json::to_string_pretty(manifest)
            .map_err(|error| ManifestRepositoryError::StorageFailed(error.to_string()))?;
        let temp_path = path.with_extension("json.tmp");

        {
            let mut file = fs::File::create(&temp_path)
                .map_err(|error| ManifestRepositoryError::StorageFailed(error.to_string()))?;
            file.write_all(serialized.as_bytes())
                .map_err(|error| ManifestRepositoryError::StorageFailed(error.to_string()))?;
            file.sync_all()
                .map_err(|error| ManifestRepositoryError::StorageFailed(error.to_string()))?;
        }

        fs::rename(&temp_path, &path)
            .map_err(|error| ManifestRepositoryError::StorageFailed(error.to_string()))
    }
}

impl InstallManifestRepository for JsonInstallManifestRepository {
    fn save_manifest(&self, manifest: &InstallManifest) -> ManifestRepositoryResult<()> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| ManifestRepositoryError::StorageFailed("manifest lock poisoned".to_owned()))?;
        self.save_file(self.manifest_path(&manifest.id), manifest)
    }

    fn load_manifest(&self, id: &InstallManifestId) -> ManifestRepositoryResult<Option<InstallManifest>> {
        let path = self.manifest_path(id);
        if !path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(path)
            .map_err(|error| ManifestRepositoryError::StorageFailed(error.to_string()))?;
        serde_json::from_slice(&bytes).map(Some).map_err(|_| ManifestRepositoryError::StorageCorrupted)
    }

    fn list_manifests(&self) -> ManifestRepositoryResult<Vec<InstallManifest>> {
        if !self.root_dir.exists() {
            return Ok(Vec::new());
        }

        let mut manifests = Vec::new();
        for entry in fs::read_dir(&self.root_dir)
            .map_err(|error| ManifestRepositoryError::StorageFailed(error.to_string()))?
        {
            let entry = entry.map_err(|error| ManifestRepositoryError::StorageFailed(error.to_string()))?;
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }

            let bytes = fs::read(entry.path())
                .map_err(|error| ManifestRepositoryError::StorageFailed(error.to_string()))?;
            let manifest = serde_json::from_slice(&bytes)
                .map_err(|_| ManifestRepositoryError::StorageCorrupted)?;
            manifests.push(manifest);
        }

        manifests.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        Ok(manifests)
    }
}

fn sanitize_file_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '_' })
        .collect()
}
```

- [ ] **Step 3: Export repository**

Modify `src-tauri/crates/hmm-infra/src/lib.rs`:

```rust
mod install_manifest_repository;

pub use install_manifest_repository::JsonInstallManifestRepository;
```

Keep existing module declarations and exports.

- [ ] **Step 4: Run repository tests**

Run:

```powershell
cargo test -p hmm-infra manifest_repository
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit manifest repository**

Run:

```powershell
git add src-tauri/crates/hmm-infra/src/install_manifest_repository.rs src-tauri/crates/hmm-infra/src/lib.rs
git commit -m "feat: 添加安装清单仓储"
```

## Task 7: Copy Install Backend

**Files:**

- Create: `src-tauri/crates/hmm-infra/src/install_backend.rs`
- Modify: `src-tauri/crates/hmm-infra/src/lib.rs`
- Test: `cargo test -p hmm-infra copy_install_backend`

- [ ] **Step 1: Add backend tests**

Create `src-tauri/crates/hmm-infra/src/install_backend.rs` with tests:

```rust
use hmm_core::*;
use hmm_ports::{InstallBackend, InstallBackendError, InstallCommitRequest};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::GameId;

    fn temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "hmm-copy-install-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ))
    }

    fn target(value: &str) -> InstallTargetPath {
        InstallTargetPath::parse(value, &["nativePC"]).expect("target")
    }

    fn plan_for(source_ref: &str, target_path: InstallTargetPath) -> InstallPlan {
        InstallPlan::new(
            InstallPlanId::new("plan-1"),
            GameId::mhw(),
            "mhw-default".to_owned(),
            ProfileId::new("default"),
        )
        .with_action(InstallAction::ValidateGameInstance)
        .with_action(InstallAction::BackupExistingFile {
            target_path: target_path.clone(),
        })
        .with_action(InstallAction::CopyFile {
            source_ref: SourceRef::new(source_ref),
            target_path,
            source_hash: FileHash::new("sha256", "expected"),
        })
        .with_action(InstallAction::WriteManifest)
    }

    fn manifest(status: InstallManifestStatus) -> InstallManifest {
        InstallManifest {
            id: InstallManifestId::new("manifest-1"),
            plan_id: InstallPlanId::new("plan-1"),
            game_id: GameId::mhw(),
            game_instance_id: "mhw-default".to_owned(),
            profile_id: ProfileId::new("default"),
            mod_id: ModId::new("mod-a"),
            backend: InstallBackendKind::Copy,
            status,
            files: Vec::new(),
            backups: Vec::new(),
        }
    }

    #[test]
    fn copy_install_backend_installs_new_file() {
        let game_root = temp_root("new-game");
        let source_root = temp_root("new-source");
        let backup_root = temp_root("new-backup");
        fs::create_dir_all(source_root.join("mod-a")).expect("source dir");
        fs::write(source_root.join("mod-a/file.bin"), b"mod file").expect("source file");

        let backend = CopyInstallBackend;
        let result = backend.commit(InstallCommitRequest {
            game_root: game_root.clone(),
            source_root,
            backup_root,
            plan: plan_for("mod-a/file.bin", target("nativePC/common/file.bin")),
            manifest: manifest(InstallManifestStatus::Planned),
        }).expect("commit");

        assert_eq!(result.status, InstallManifestStatus::Completed);
        assert_eq!(
            fs::read(game_root.join("nativePC/common/file.bin")).expect("installed"),
            b"mod file"
        );
        assert!(matches!(result.files[0].previous_state, PreviousFileState::Missing));
    }

    #[test]
    fn copy_install_backend_backs_up_overwritten_file() {
        let game_root = temp_root("overwrite-game");
        let source_root = temp_root("overwrite-source");
        let backup_root = temp_root("overwrite-backup");
        fs::create_dir_all(game_root.join("nativePC/common")).expect("game dir");
        fs::write(game_root.join("nativePC/common/file.bin"), b"original").expect("original");
        fs::create_dir_all(source_root.join("mod-a")).expect("source dir");
        fs::write(source_root.join("mod-a/file.bin"), b"mod file").expect("source file");

        let backend = CopyInstallBackend;
        let result = backend.commit(InstallCommitRequest {
            game_root,
            source_root,
            backup_root: backup_root.clone(),
            plan: plan_for("mod-a/file.bin", target("nativePC/common/file.bin")),
            manifest: manifest(InstallManifestStatus::Planned),
        }).expect("commit");

        assert_eq!(result.backups.len(), 1);
        assert!(backup_root.exists());
    }

    #[test]
    fn copy_install_backend_rolls_back_when_source_missing() {
        let game_root = temp_root("rollback-game");
        let source_root = temp_root("rollback-source");
        let backup_root = temp_root("rollback-backup");
        fs::create_dir_all(&game_root).expect("game root");
        fs::create_dir_all(&source_root).expect("source root");

        let backend = CopyInstallBackend;
        let error = backend.commit(InstallCommitRequest {
            game_root: game_root.clone(),
            source_root,
            backup_root,
            plan: plan_for("missing/file.bin", target("nativePC/common/file.bin")),
            manifest: manifest(InstallManifestStatus::Planned),
        }).expect_err("missing source should fail");

        assert!(matches!(error, InstallBackendError::SourceMissing(_)));
        assert!(!game_root.join("nativePC/common/file.bin").exists());
    }
}
```

- [ ] **Step 2: Implement backend skeleton and hash helper**

Above tests in `install_backend.rs`, add:

```rust
pub struct CopyInstallBackend;

impl InstallBackend for CopyInstallBackend {
    fn commit(&self, request: InstallCommitRequest) -> Result<InstallManifest, InstallBackendError> {
        if !request.plan.is_committable() {
            return Err(InstallBackendError::PlanBlocked);
        }

        fs::create_dir_all(&request.game_root)
            .map_err(|error| InstallBackendError::GameDirectoryInvalid(error.to_string()))?;
        fs::create_dir_all(&request.backup_root)
            .map_err(|error| InstallBackendError::FileOperationFailed(error.to_string()))?;

        let mut manifest = InstallManifest {
            status: InstallManifestStatus::Committing,
            ..request.manifest
        };
        let mut rollback = RollbackJournal::default();

        for action in &request.plan.actions {
            match action {
                InstallAction::ValidateGameInstance | InstallAction::WriteManifest => {}
                InstallAction::CreateDirectory { target_dir } => {
                    let path = request.game_root.join(target_dir.to_path_buf());
                    fs::create_dir_all(&path)
                        .map_err(|error| InstallBackendError::FileOperationFailed(error.to_string()))?;
                }
                InstallAction::BackupExistingFile { target_path } => {
                    let target = request.game_root.join(target_path.to_path_buf());
                    if target.exists() {
                        let backup_ref = backup_ref_for(target_path);
                        let backup = request.backup_root.join(&backup_ref);
                        if let Some(parent) = backup.parent() {
                            fs::create_dir_all(parent)
                                .map_err(|error| InstallBackendError::FileOperationFailed(error.to_string()))?;
                        }
                        fs::copy(&target, &backup)
                            .map_err(|error| InstallBackendError::FileOperationFailed(error.to_string()))?;
                        rollback.backups.push((target.clone(), backup.clone()));
                        manifest.backups.push(BackupRecord {
                            target_path: target_path.clone(),
                            backup_ref,
                            original_hash: hash_file(&target)?,
                            original_size: fs::metadata(&target)
                                .map_err(|error| InstallBackendError::FileOperationFailed(error.to_string()))?
                                .len(),
                        });
                    }
                }
                InstallAction::CopyFile {
                    source_ref,
                    target_path,
                    source_hash,
                } => {
                    let source = request.source_root.join(source_ref.as_str());
                    if !source.is_file() {
                        rollback.restore().map_err(InstallBackendError::RollbackFailed)?;
                        return Err(InstallBackendError::SourceMissing(source_ref.as_str().to_owned()));
                    }

                    let target = request.game_root.join(target_path.to_path_buf());
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent)
                            .map_err(|error| InstallBackendError::FileOperationFailed(error.to_string()))?;
                    }

                    let previous_state = previous_state_for(target_path, &manifest.backups);
                    fs::copy(&source, &target)
                        .map_err(|error| InstallBackendError::FileOperationFailed(error.to_string()))?;
                    rollback.created_or_changed.push(target.clone());

                    manifest.files.push(InstalledFile {
                        target_path: target_path.clone(),
                        source_ref: source_ref.clone(),
                        source_hash: source_hash.clone(),
                        installed_hash: hash_file(&target)?,
                        install_kind: InstallKind::Copy,
                        previous_state,
                    });
                }
                InstallAction::RemoveFile { target_path } => {
                    let target = request.game_root.join(target_path.to_path_buf());
                    if target.exists() {
                        fs::remove_file(&target)
                            .map_err(|error| InstallBackendError::FileOperationFailed(error.to_string()))?;
                    }
                }
            }
        }

        manifest.status = InstallManifestStatus::Completed;
        Ok(manifest)
    }

    fn uninstall(&self, game_root: &Path, manifest: &InstallManifest) -> Result<InstallManifest, InstallBackendError> {
        uninstall_manifest(game_root, manifest)
    }
}
```

- [ ] **Step 3: Implement helper functions**

Continue in `install_backend.rs`:

```rust
#[derive(Default)]
struct RollbackJournal {
    backups: Vec<(PathBuf, PathBuf)>,
    created_or_changed: Vec<PathBuf>,
}

impl RollbackJournal {
    fn restore(&self) -> Result<(), String> {
        for path in self.created_or_changed.iter().rev() {
            if path.exists() {
                fs::remove_file(path).map_err(|error| error.to_string())?;
            }
        }

        for (target, backup) in self.backups.iter().rev() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::copy(backup, target).map_err(|error| error.to_string())?;
        }

        Ok(())
    }
}

fn backup_ref_for(target_path: &InstallTargetPath) -> String {
    format!("{}.bak", target_path.as_str().replace('/', "__"))
}

fn previous_state_for(target_path: &InstallTargetPath, backups: &[BackupRecord]) -> PreviousFileState {
    backups
        .iter()
        .find(|backup| backup.target_path == *target_path)
        .map(|backup| PreviousFileState::BackedUp {
            backup_ref: backup.backup_ref.clone(),
            hash: backup.original_hash.clone(),
        })
        .unwrap_or(PreviousFileState::Missing)
}

fn hash_file(path: &Path) -> Result<FileHash, InstallBackendError> {
    let mut file = fs::File::open(path)
        .map_err(|error| InstallBackendError::FileOperationFailed(error.to_string()))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|error| InstallBackendError::FileOperationFailed(error.to_string()))?;
    let value = buffer
        .iter()
        .fold(0u64, |acc, byte| acc.wrapping_mul(31).wrapping_add(u64::from(*byte)));
    Ok(FileHash::new("hmm-simple", format!("{value:016x}")))
}

fn uninstall_manifest(
    game_root: &Path,
    manifest: &InstallManifest,
) -> Result<InstallManifest, InstallBackendError> {
    for file in &manifest.files {
        let target = game_root.join(file.target_path.to_path_buf());
        match &file.previous_state {
            PreviousFileState::Missing => {
                if target.exists() {
                    fs::remove_file(&target)
                        .map_err(|error| InstallBackendError::FileOperationFailed(error.to_string()))?;
                }
            }
            PreviousFileState::BackedUp { backup_ref, .. } => {
                let backup = game_root.join(".hmm-backups").join(backup_ref);
                if backup.exists() {
                    if let Some(parent) = target.parent() {
                        fs::create_dir_all(parent)
                            .map_err(|error| InstallBackendError::FileOperationFailed(error.to_string()))?;
                    }
                    fs::copy(backup, target)
                        .map_err(|error| InstallBackendError::FileOperationFailed(error.to_string()))?;
                }
            }
        }
    }

    Ok(InstallManifest {
        status: InstallManifestStatus::RolledBack,
        ..manifest.clone()
    })
}
```

The hash helper is intentionally simple for MVP tests. Replace it with a real SHA-256 implementation in a focused follow-up before using it for trust decisions or diagnostics.

- [ ] **Step 4: Export copy backend**

Modify `src-tauri/crates/hmm-infra/src/lib.rs`:

```rust
mod install_backend;

pub use install_backend::CopyInstallBackend;
```

Keep existing module declarations and exports.

- [ ] **Step 5: Run backend tests**

Run:

```powershell
cargo test -p hmm-infra copy_install_backend
```

Expected:

```text
test result: ok
```

- [ ] **Step 6: Commit copy backend**

Run:

```powershell
git add src-tauri/crates/hmm-infra/src/install_backend.rs src-tauri/crates/hmm-infra/src/lib.rs
git commit -m "feat: 实现安全复制安装后端"
```

## Task 8: App Commit and Uninstall Services

**Files:**

- Modify: `src-tauri/crates/hmm-app/src/install.rs`
- Modify: `src-tauri/crates/hmm-app/src/lib.rs`
- Test: `cargo test -p hmm-app commit_install_plan`

- [ ] **Step 1: Add commit service tests**

Append tests in `src-tauri/crates/hmm-app/src/install.rs`:

```rust
#[cfg(test)]
mod commit_tests {
    use super::*;
    use hmm_ports::{InstallBackend, InstallBackendError, InstallCommitRequest, InstallManifestRepository, ManifestRepositoryResult};
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    struct FakeBackend {
        committed: Mutex<bool>,
    }

    impl InstallBackend for FakeBackend {
        fn commit(&self, request: InstallCommitRequest) -> Result<InstallManifest, InstallBackendError> {
            *self.committed.lock().expect("lock") = true;
            Ok(InstallManifest {
                status: InstallManifestStatus::Completed,
                ..request.manifest
            })
        }

        fn uninstall(&self, _game_root: &Path, manifest: &InstallManifest) -> Result<InstallManifest, InstallBackendError> {
            Ok(InstallManifest {
                status: InstallManifestStatus::RolledBack,
                ..manifest.clone()
            })
        }
    }

    struct FakeManifestRepository {
        saved: Mutex<Vec<InstallManifest>>,
    }

    impl InstallManifestRepository for FakeManifestRepository {
        fn save_manifest(&self, manifest: &InstallManifest) -> ManifestRepositoryResult<()> {
            self.saved.lock().expect("lock").push(manifest.clone());
            Ok(())
        }

        fn load_manifest(&self, _id: &InstallManifestId) -> ManifestRepositoryResult<Option<InstallManifest>> {
            Ok(None)
        }

        fn list_manifests(&self) -> ManifestRepositoryResult<Vec<InstallManifest>> {
            Ok(self.saved.lock().expect("lock").clone())
        }
    }

    #[test]
    fn commit_service_saves_completed_manifest() {
        let backend = Arc::new(FakeBackend {
            committed: Mutex::new(false),
        });
        let repo = Arc::new(FakeManifestRepository {
            saved: Mutex::new(Vec::new()),
        });
        let service = InstallCommitService::new(backend.clone(), repo.clone());

        let result = service.commit_install_plan(CommitInstallPlanRequest {
            game_root: PathBuf::from("game"),
            source_root: PathBuf::from("source"),
            backup_root: PathBuf::from("backup"),
            plan: InstallPlan::new(
                InstallPlanId::new("plan-1"),
                GameId::mhw(),
                "mhw-default".to_owned(),
                ProfileId::new("default"),
            )
            .with_action(InstallAction::ValidateGameInstance)
            .with_action(InstallAction::WriteManifest),
            manifest: InstallManifest {
                id: InstallManifestId::new("manifest-1"),
                plan_id: InstallPlanId::new("plan-1"),
                game_id: GameId::mhw(),
                game_instance_id: "mhw-default".to_owned(),
                profile_id: ProfileId::new("default"),
                mod_id: ModId::new("mod-a"),
                backend: InstallBackendKind::Copy,
                status: InstallManifestStatus::Planned,
                files: Vec::new(),
                backups: Vec::new(),
            },
        }).expect("commit");

        assert_eq!(result.status, InstallManifestStatus::Completed);
        assert_eq!(repo.saved.lock().expect("lock").len(), 1);
        assert!(*backend.committed.lock().expect("lock"));
    }
}
```

- [ ] **Step 2: Implement commit service**

Add to `install.rs`:

```rust
use hmm_ports::{InstallBackend, InstallBackendError, InstallCommitRequest, InstallManifestRepository, ManifestRepositoryError};
use std::path::PathBuf;

#[derive(Debug, Error)]
pub enum InstallCommitError {
    #[error("backend failed: {0}")]
    Backend(#[from] InstallBackendError),
    #[error("manifest repository failed: {0}")]
    ManifestRepository(#[from] ManifestRepositoryError),
}

pub struct CommitInstallPlanRequest {
    pub game_root: PathBuf,
    pub source_root: PathBuf,
    pub backup_root: PathBuf,
    pub plan: InstallPlan,
    pub manifest: InstallManifest,
}

pub struct InstallCommitService {
    backend: Arc<dyn InstallBackend>,
    manifest_repository: Arc<dyn InstallManifestRepository>,
}

impl InstallCommitService {
    pub fn new(
        backend: Arc<dyn InstallBackend>,
        manifest_repository: Arc<dyn InstallManifestRepository>,
    ) -> Self {
        Self {
            backend,
            manifest_repository,
        }
    }

    pub fn commit_install_plan(
        &self,
        request: CommitInstallPlanRequest,
    ) -> Result<InstallManifest, InstallCommitError> {
        let committed = self.backend.commit(InstallCommitRequest {
            game_root: request.game_root,
            source_root: request.source_root,
            backup_root: request.backup_root,
            plan: request.plan,
            manifest: request.manifest,
        })?;

        self.manifest_repository.save_manifest(&committed)?;
        Ok(committed)
    }
}
```

- [ ] **Step 3: Export commit service**

Modify `src-tauri/crates/hmm-app/src/lib.rs`:

```rust
pub use install::{
    BuildInstallPlanRequest, CommitInstallPlanRequest, InstallCommitError, InstallCommitService,
    InstallPlanningError, InstallPlanningService,
};
```

- [ ] **Step 4: Run app commit tests**

Run:

```powershell
cargo test -p hmm-app commit_install_plan
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit app service**

Run:

```powershell
git add src-tauri/crates/hmm-app/src/install.rs src-tauri/crates/hmm-app/src/lib.rs
git commit -m "feat: 编排安装提交和清单保存"
```

## Task 9: Tauri DTO and Commands for Backend MVP

**Files:**

- Modify: `src-tauri/src/dto.rs`
- Create: `src-tauri/src/install_commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/state.rs`
- Test: `cargo test -p hmm-tauri install_commands`

- [ ] **Step 1: Add install DTOs**

In `src-tauri/src/dto.rs`, add:

```rust
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFileProviderDto {
    pub mod_id: String,
    pub package_file_id: String,
    pub source_ref: String,
    pub source_hash: String,
    pub target_path: String,
    pub priority: i32,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlanDto {
    pub id: String,
    pub is_committable: bool,
    pub action_count: usize,
    pub conflicts: Vec<InstallConflictDto>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallConflictDto {
    pub target_path: String,
    pub providers: Vec<String>,
    pub reason: String,
}
```

Add mapping helpers:

```rust
pub fn install_plan_to_dto(plan: hmm_core::InstallPlan) -> InstallPlanDto {
    InstallPlanDto {
        id: plan.id.as_str().to_owned(),
        is_committable: plan.is_committable(),
        action_count: plan.actions.len(),
        conflicts: plan
            .conflicts
            .into_iter()
            .map(|conflict| InstallConflictDto {
                target_path: conflict.target_path.as_str().to_owned(),
                providers: conflict
                    .providers
                    .into_iter()
                    .map(|provider| provider.as_str().to_owned())
                    .collect(),
                reason: format!("{:?}", conflict.reason),
            })
            .collect(),
    }
}
```

- [ ] **Step 2: Create commands**

Create `src-tauri/src/install_commands.rs`:

```rust
use crate::dto::{install_plan_to_dto, InstallPlanDto, ModFileProviderDto};
use crate::game_setup_commands::CommandErrorDto;
use crate::state::AppState;
use hmm_app::BuildInstallPlanRequest;
use hmm_core::{
    FileHash, GameId, InstallKind, InstallTargetPath, ModFileProvider, ModId, PackageFileId,
    ProfileId, SourceRef,
};
use tauri::State;

#[tauri::command]
pub fn preview_install_plan(
    game_id: String,
    game_instance_id: String,
    profile_id: String,
    mod_id: String,
    providers: Vec<ModFileProviderDto>,
    state: State<'_, AppState>,
) -> Result<InstallPlanDto, CommandErrorDto> {
    let game_id = parse_game_id(game_id)?;
    let service = state.install_planning.lock().map_err(|_| CommandErrorDto {
        code: "unknown".to_owned(),
        message: "install planning state lock failed".to_owned(),
    })?;

    let providers = providers
        .into_iter()
        .map(|provider| dto_to_provider(provider, &["nativePC", "MonsterLoader.dll", "dinput8.dll"]))
        .collect::<Result<Vec<_>, _>>()?;

    service
        .build_install_plan(BuildInstallPlanRequest {
            game_id,
            game_instance_id,
            profile_id: ProfileId::new(profile_id),
            mod_id: ModId::new(mod_id),
            providers,
        })
        .map(install_plan_to_dto)
        .map_err(|error| CommandErrorDto {
            code: "install_plan_failed".to_owned(),
            message: error.to_string(),
        })
}

fn parse_game_id(value: String) -> Result<GameId, CommandErrorDto> {
    GameId::parse(value).map_err(|error| CommandErrorDto {
        code: "unsupported_game".to_owned(),
        message: error.to_string(),
    })
}

fn dto_to_provider(
    value: ModFileProviderDto,
    allowed_roots: &[&str],
) -> Result<ModFileProvider, CommandErrorDto> {
    let target_path =
        InstallTargetPath::parse(&value.target_path, allowed_roots).map_err(|error| {
            CommandErrorDto {
                code: "invalid_install_target".to_owned(),
                message: error.to_string(),
            }
        })?;

    Ok(ModFileProvider {
        mod_id: ModId::new(value.mod_id),
        package_file_id: PackageFileId::new(value.package_file_id),
        source_ref: SourceRef::new(value.source_ref),
        source_hash: FileHash::new("sha256", value.source_hash),
        target_path,
        priority: value.priority,
        install_kind: InstallKind::Copy,
        replacement_binding_id: None,
        generated_from: None,
    })
}
```

This command previews plans only. Commit command wiring can be added after source cache and manifest storage paths are finalized in app state.

- [ ] **Step 3: Add install planning service to state**

Modify `src-tauri/src/state.rs` to store:

```rust
pub install_planning: Mutex<InstallPlanningService>,
```

Initialize it with the same adapter list used by `GameSetupService`. Keep `hmm-games-mhw` as the adapter source; do not put target roots in frontend code.

- [ ] **Step 4: Register command**

Modify `src-tauri/src/lib.rs`:

```rust
mod install_commands;
```

Register `preview_install_plan` in `tauri::generate_handler!`.

- [ ] **Step 5: Run Tauri tests**

Run:

```powershell
cargo test -p hmm-tauri install_commands
cargo test -p hmm-tauri
```

Expected:

```text
test result: ok
```

- [ ] **Step 6: Commit Tauri preview command**

Run:

```powershell
git add src-tauri/src/dto.rs src-tauri/src/install_commands.rs src-tauri/src/lib.rs src-tauri/src/state.rs
git commit -m "feat: 添加安装计划预览命令"
```

## Task 10: Minimal Frontend Typed API

**Files:**

- Create: `src/features/mods/modInstallTypes.ts`
- Create: `src/features/mods/modInstallApi.ts`
- Modify: `src/shared/api/tauri.ts`
- Test: `cmd /c corepack pnpm run typecheck`

- [ ] **Step 1: Add frontend types**

Create `src/features/mods/modInstallTypes.ts`:

```ts
export type ModFileProviderInput = {
  modId: string;
  packageFileId: string;
  sourceRef: string;
  sourceHash: string;
  targetPath: string;
  priority: number;
};

export type InstallConflict = {
  targetPath: string;
  providers: string[];
  reason: string;
};

export type InstallPlanPreview = {
  id: string;
  isCommittable: boolean;
  actionCount: number;
  conflicts: InstallConflict[];
};
```

- [ ] **Step 2: Add typed API**

Create `src/features/mods/modInstallApi.ts`:

```ts
import { invokeCommand } from "../../shared/api/tauri";
import type { InstallPlanPreview, ModFileProviderInput } from "./modInstallTypes";

export type PreviewInstallPlanInput = {
  gameId: string;
  gameInstanceId: string;
  profileId: string;
  modId: string;
  providers: ModFileProviderInput[];
};

export function previewInstallPlan(input: PreviewInstallPlanInput) {
  return invokeCommand<InstallPlanPreview>("preview_install_plan", input);
}
```

- [ ] **Step 3: Ensure shared invoke supports typed arguments**

If `src/shared/api/tauri.ts` currently only accepts zero-argument commands, change it to:

```ts
import { invoke } from "@tauri-apps/api/core";

export function invokeCommand<T>(command: string, args?: Record<string, unknown>) {
  return invoke<T>(command, args);
}
```

If it already supports typed args, keep the existing implementation.

- [ ] **Step 4: Run frontend typecheck**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected:

```text
No TypeScript errors
```

- [ ] **Step 5: Commit typed API**

Run:

```powershell
git add src/features/mods/modInstallTypes.ts src/features/mods/modInstallApi.ts src/shared/api/tauri.ts
git commit -m "feat: 添加安装计划前端 API"
```

## Task 11: End-to-End Verification

**Files:**

- Verify: whole workspace

- [ ] **Step 1: Check whitespace-sensitive diff**

Run:

```powershell
git diff --check
```

Expected: no output.

- [ ] **Step 2: Run Rust checks**

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

- [ ] **Step 3: Run frontend checks**

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

- [ ] **Step 4: Run unified verification**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected:

```text
Verification passed.
```

- [ ] **Step 5: Manual safety review**

Confirm:

- Tests only use temp directories.
- No test reads or writes a real game directory.
- No test reads or writes real player saves.
- No real third-party Mod package is added to the repository.
- No command writes files without `InstallPlan`.
- No uninstall path guesses files from the current package; it uses manifest data.
- No frontend code hardcodes game-specific installation paths.

## Follow-Up Implementation Plans

After this MVP is merged and verified, split these into separate plans:

- Archive import and sandbox extraction.
- Real SHA-256 hashing and audit log integration.
- Manifest-backed uninstall and repair UI.
- Profile enable/disable and batch apply.
- Replacement target catalog and retarget staging.
- Virtual mapping backend with permission probing and repair scan.

Do not implement these follow-ups in the MVP branch unless the task is explicitly re-scoped.

## PR Handoff

- [ ] **Step 1: Confirm branch status**

Run:

```powershell
git status --short --branch --untracked-files=all
```

Expected:

```text
## codex/mod-installation-mvp
```

The working tree should contain only files for this implementation. Runtime planning files, generated caches, fake game directories, backup directories and local logs must not be staged.

- [ ] **Step 2: Push branch**

Run:

```powershell
git push -u origin codex/mod-installation-mvp
```

- [ ] **Step 3: Create PR**

Run:

```powershell
gh pr create --base main --head codex/mod-installation-mvp --title "[codex] 实现 Mod 安装 MVP 后端链路" --body "## 改动`n- 添加安装目标路径、文件层栈、安装计划和安装清单模型`n- 添加安装 ports、计划构建服务、复制安装后端和 JSON manifest 仓储`n- 添加安装计划预览命令和前端 typed API`n`n## 验证`n- cargo test --workspace`n- cargo check --workspace`n- cmd /c corepack pnpm run typecheck`n- cmd /c corepack pnpm run lint`n- cmd /c corepack pnpm run build`n- powershell -NoProfile -ExecutionPolicy Bypass -File .\\scripts\\verify.ps1`n`n## 安全边界`n- 测试只使用临时目录`n- 不读写真实游戏目录、真实存档或真实第三方 Mod 包`n- MVP 只实现安全物理复制，不实现虚拟映射"
```

## Self-Review Checklist

- [ ] `hmm-core` 没有使用 `std::fs`、Tauri API、JSON repository 或平台 API。
- [ ] `hmm-app` 只依赖 traits，不直接调用真实文件系统。
- [ ] 游戏专属安装根只在 adapter crate 或 adapter 测试中出现。
- [ ] `CopyInstallBackend` 覆盖前备份，失败时尝试回滚。
- [ ] `InstallManifest` 记录 installed files、backups、backend、status 和 previous state。
- [ ] 卸载基于 manifest，不根据当前 Mod 包重新推测文件。
- [ ] Tauri command 只做 DTO 转换和用例转发。
- [ ] 前端 API 不拼接游戏目录路径。
- [ ] 测试不依赖真实游戏安装、真实存档或真实第三方 Mod 包。
- [ ] 最终回复准确记录已执行验证和未执行验证。
