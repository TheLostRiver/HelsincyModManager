# Steam Library Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 Steam 游戏目录自动扫描候选列表，让玩家从 Steam 发现的 MHW:I 候选目录中选择并保存游戏目录。

**Architecture:** 基础设施层负责 Steam root、VDF 和 app manifest 读取；应用层负责把 raw candidate 交给游戏 adapter 校验；Tauri command 只做 DTO 转换；前端只展示候选并复用现有保存流程。Steam 和平台细节不能进入 React 组件，MHW:I 规则不能进入通用 discovery 逻辑。

**Tech Stack:** Tauri 2、React 19、TypeScript、Rust workspace、serde、thiserror、临时目录测试、PowerShell verify 脚本。

---

## Scope

本计划实现：

- `scan_game_candidates("mhw")` 返回候选列表。
- Steam library 扫描读取 `libraryfolders.vdf` 和 `appmanifest_582010.acf`。
- 候选目录经 MHW:I adapter 校验后返回 `isValid`、`confidence`、`errors` 和 `evidence`。
- 前端展示候选列表，允许玩家选择有效候选。
- 没有 Steam 或没有候选时保留手动选择兜底。

本计划不实现：

- 进程扫描。
- 一键启动游戏。
- 自动保存候选。
- 写入游戏目录。
- 读取或修改存档。
- Steam Deck 实机验证。

## Target File Structure

```text
src-tauri/
  src/
    dto.rs
    game_setup_commands.rs
    state.rs
  crates/
    hmm-ports/src/
      game_setup.rs
    hmm-games-mhw/src/
      lib.rs
    hmm-app/src/
      game_setup.rs
    hmm-infra/src/
      lib.rs
      game_discovery.rs
      steam_discovery/
        mod.rs
        key_values.rs
        library_manifest.rs
        root_provider.rs

src/
  features/
    game-setup/
      GameDirectoryActions.tsx
      GameDirectoryCandidateList.css
      GameDirectoryCandidateList.tsx
      gameSetupApi.ts
      gameSetupTypes.ts
      gameSetupViewModel.ts
      useGameSetup.ts
    dashboard/
      DashboardHeroCard.tsx
      DashboardPage.tsx
```

职责锁定：

- `hmm-ports`：定义 discovery request、candidate source 和 raw candidate。
- `hmm-games-mhw`：只声明 MHW:I 的 Steam app id 和目录校验规则。
- `hmm-infra/steam_discovery`：只处理 Steam 文件结构和 KeyValues 解析。
- `hmm-app`：候选校验、排序、去重。
- `src-tauri/src/dto.rs`：Rust 结果到前端 DTO。
- `features/game-setup`：前端 API、状态、候选列表和动作。
- `dashboard`：组合 game setup feature，不处理 Steam 规则。

## Task 0: Preflight

**Files:**

- Read: `AGENTS.md`
- Read: `docs/ARCHITECTURE.md`
- Read: `docs/TESTING.md`
- Read: `docs/superpowers/specs/2026-06-06-steam-library-discovery-design.md`
- Read: `src-tauri/crates/hmm-ports/src/game_setup.rs`
- Read: `src-tauri/crates/hmm-app/src/game_setup.rs`
- Read: `src-tauri/crates/hmm-infra/src/game_discovery.rs`
- Read: `src/features/game-setup/useGameSetup.ts`

- [ ] **Step 1: Confirm branch and clean tree**

Run:

```powershell
git status --short --branch --untracked-files=all
```

Expected:

```text
## codex/steam-library-discovery
```

There must be no unrelated modified or untracked files before implementation starts.

- [ ] **Step 2: Confirm current scan behavior**

Run:

```powershell
cargo test -p hmm-app scan_candidates_returns_explicit_not_implemented
cargo test -p hmm-infra scan_returns_explicit_not_implemented
cmd /c corepack pnpm run typecheck
```

Expected:

```text
scan_candidates_returns_explicit_not_implemented ... ok
scan_returns_explicit_not_implemented ... ok
No TypeScript errors
```

These tests define the behavior that will be replaced by real candidate scanning.

## Task 1: Ports Discovery Models

**Files:**

- Modify: `src-tauri/crates/hmm-ports/src/game_setup.rs`
- Test: `cargo test -p hmm-ports`

- [ ] **Step 1: Add discovery request and source models**

In `src-tauri/crates/hmm-ports/src/game_setup.rs`, replace the existing `GameCandidate` and `GameDiscoveryService` definitions with this shape while preserving existing repository and probe traits:

```rust
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
```

- [ ] **Step 2: Add Steam app id to GameAdapter**

In the same file, add a default method to `GameAdapter`:

```rust
pub trait GameAdapter: Send + Sync {
    fn game_id(&self) -> GameId;
    fn display_name(&self) -> &'static str;
    fn validate_directory(&self, probe: &dyn GameDirectoryProbe) -> GameDirectoryValidation;

    fn steam_app_id(&self) -> Option<u32> {
        None
    }
}
```

- [ ] **Step 3: Run ports check**

Run:

```powershell
cargo test -p hmm-ports
```

Expected:

```text
test result: ok
```

- [ ] **Step 4: Commit ports update**

Run:

```powershell
git add src-tauri/crates/hmm-ports/src/game_setup.rs
git commit -m "feat: 扩展游戏目录发现接口"
```

## Task 2: MHW:I Adapter Steam App Id

**Files:**

- Modify: `src-tauri/crates/hmm-games-mhw/src/lib.rs`
- Test: `cargo test -p hmm-games-mhw`

- [ ] **Step 1: Add app id constant and trait method**

In `src-tauri/crates/hmm-games-mhw/src/lib.rs`, add:

```rust
const STEAM_APP_ID: u32 = 582010;
```

Then implement the new trait method:

```rust
fn steam_app_id(&self) -> Option<u32> {
    Some(STEAM_APP_ID)
}
```

- [ ] **Step 2: Add adapter test**

Add this test to the existing test module:

```rust
#[test]
fn adapter_reports_steam_app_id() {
    let adapter = MonsterHunterWorldAdapter;
    assert_eq!(adapter.steam_app_id(), Some(582010));
}
```

- [ ] **Step 3: Run MHW adapter tests**

Run:

```powershell
cargo test -p hmm-games-mhw
```

Expected:

```text
test result: ok
```

- [ ] **Step 4: Commit adapter update**

Run:

```powershell
git add src-tauri/crates/hmm-games-mhw/src/lib.rs
git commit -m "feat: 声明 MHW Steam 应用标识"
```

## Task 3: Steam KeyValues Parser

**Files:**

- Create: `src-tauri/crates/hmm-infra/src/steam_discovery/mod.rs`
- Create: `src-tauri/crates/hmm-infra/src/steam_discovery/key_values.rs`
- Modify: `src-tauri/crates/hmm-infra/src/lib.rs`
- Test: `cargo test -p hmm-infra steam_key_values`

- [ ] **Step 1: Create module entry**

Create `src-tauri/crates/hmm-infra/src/steam_discovery/mod.rs`:

```rust
mod key_values;
```

Task 3 只声明 `key_values`，因为 `library_manifest.rs` 和 `root_provider.rs` 此时还不存在。不要在 Task 3 提前声明这两个模块；否则 `cargo test -p hmm-infra steam_key_values` 会在解析器测试运行前编译失败。

- [ ] **Step 2: Add parser public API**

Create `src-tauri/crates/hmm-infra/src/steam_discovery/key_values.rs` with a small parser that exposes:

```rust
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyValueNode {
    Text(String),
    Object(BTreeMap<String, KeyValueNode>),
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KeyValuesError {
    #[error("unexpected end of input")]
    UnexpectedEnd,
    #[error("expected quoted string")]
    ExpectedQuotedString,
    #[error("expected object")]
    ExpectedObject,
    #[error("unexpected token: {0}")]
    UnexpectedToken(String),
}

pub fn parse_key_values(input: &str) -> Result<KeyValueNode, KeyValuesError> {
    KeyValuesParser::new(input).parse()
}
```

The parser implementation must:

- Parse quoted strings.
- Parse nested `{ ... }` objects.
- Ignore whitespace between tokens.
- Return errors for unclosed quotes and unclosed objects.
- Keep parsing logic inside this file.

- [ ] **Step 3: Add parser tests**

Add tests in `key_values.rs` under `#[cfg(test)]`:

```rust
#[test]
fn steam_key_values_parses_nested_objects() {
    let parsed = parse_key_values(
        r#"
        "libraryfolders"
        {
            "0"
            {
                "path" "D:\\SteamLibrary"
                "apps"
                {
                    "582010" "123"
                }
            }
        }
        "#,
    )
    .expect("valid vdf");

    assert!(matches!(parsed, KeyValueNode::Object(_)));
}

#[test]
fn steam_key_values_rejects_unclosed_quote() {
    let error = parse_key_values(r#""libraryfolders" { "0"#).expect_err("invalid vdf");
    assert_eq!(error, KeyValuesError::UnexpectedEnd);
}
```

- [ ] **Step 4: Wire infra module**

Modify `src-tauri/crates/hmm-infra/src/lib.rs`:

```rust
mod steam_discovery;
```

Do not export parser internals outside infra.

- [ ] **Step 5: Run parser tests**

Run:

```powershell
cargo test -p hmm-infra steam_key_values
```

Expected:

```text
test result: ok
```

- [ ] **Step 6: Commit parser**

Run:

```powershell
git add src-tauri/crates/hmm-infra/src/steam_discovery src-tauri/crates/hmm-infra/src/lib.rs
git commit -m "feat: 添加 Steam KeyValues 解析器"
```

## Task 4: Steam Library Manifest Parsing

**Files:**

- Create: `src-tauri/crates/hmm-infra/src/steam_discovery/library_manifest.rs`
- Modify: `src-tauri/crates/hmm-infra/src/steam_discovery/mod.rs`
- Test: `cargo test -p hmm-infra steam_manifest`

- [ ] **Step 1: Add typed parser functions**

Create `src-tauri/crates/hmm-infra/src/steam_discovery/library_manifest.rs`:

```rust
use super::key_values::{parse_key_values, KeyValueNode, KeyValuesError};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamLibraryFolder {
    pub path: PathBuf,
    pub app_ids: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamAppManifest {
    pub app_id: u32,
    pub install_dir: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SteamManifestError {
    #[error("key values parse failed: {0}")]
    Parse(#[from] KeyValuesError),
    #[error("expected object: {0}")]
    ExpectedObject(&'static str),
    #[error("missing field: {0}")]
    MissingField(&'static str),
    #[error("invalid app id: {0}")]
    InvalidAppId(String),
}

pub fn parse_library_folders(input: &str) -> Result<Vec<SteamLibraryFolder>, SteamManifestError> {
    // Use parse_key_values, then extract libraryfolders -> numeric entries -> path/apps.
}

pub fn parse_app_manifest(input: &str) -> Result<SteamAppManifest, SteamManifestError> {
    // Use parse_key_values, then extract AppState -> appid/installdir.
}
```

Implement the extraction code in this file. Keep field lookup case-sensitive because Steam manifest keys are stable in generated files.

- [ ] **Step 2: Export manifest parser module**

Modify `src-tauri/crates/hmm-infra/src/steam_discovery/mod.rs`:

```rust
mod key_values;
mod library_manifest;

pub use library_manifest::{
    parse_app_manifest, parse_library_folders, SteamAppManifest, SteamLibraryFolder,
};
```

- [ ] **Step 3: Add manifest parser tests**

Add tests in `library_manifest.rs`:

```rust
#[test]
fn steam_manifest_parses_library_folders_with_target_app() {
    let folders = parse_library_folders(
        r#"
        "libraryfolders"
        {
            "0"
            {
                "path" "D:\\SteamLibrary"
                "apps"
                {
                    "582010" "123456"
                }
            }
        }
        "#,
    )
    .expect("library folders");

    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].app_ids, vec![582010]);
}

#[test]
fn steam_manifest_parses_app_manifest_install_dir() {
    let manifest = parse_app_manifest(
        r#"
        "AppState"
        {
            "appid" "582010"
            "installdir" "Monster Hunter World"
        }
        "#,
    )
    .expect("app manifest");

    assert_eq!(manifest.app_id, 582010);
    assert_eq!(manifest.install_dir, "Monster Hunter World");
}
```

- [ ] **Step 4: Run manifest tests**

Run:

```powershell
cargo test -p hmm-infra steam_manifest
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit manifest parser**

Run:

```powershell
git add src-tauri/crates/hmm-infra/src/steam_discovery/library_manifest.rs src-tauri/crates/hmm-infra/src/steam_discovery/mod.rs
git commit -m "feat: 解析 Steam library 与应用清单"
```

## Task 5: Steam Root Provider

**Files:**

- Create: `src-tauri/crates/hmm-infra/src/steam_discovery/root_provider.rs`
- Modify: `src-tauri/crates/hmm-infra/src/steam_discovery/mod.rs`
- Modify: `src-tauri/crates/hmm-infra/Cargo.toml`
- Test: `cargo test -p hmm-infra steam_root`

- [ ] **Step 1: Add root provider API**

Create `src-tauri/crates/hmm-infra/src/steam_discovery/root_provider.rs`:

```rust
use std::path::{Path, PathBuf};

pub trait SteamRootProvider: Send + Sync {
    fn steam_roots(&self) -> Vec<PathBuf>;
}

pub struct PlatformSteamRootProvider;

impl SteamRootProvider for PlatformSteamRootProvider {
    fn steam_roots(&self) -> Vec<PathBuf> {
        platform_steam_roots()
    }
}

pub fn linux_steam_roots_from_home(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".steam").join("steam"),
        home.join(".local").join("share").join("Steam"),
        home.join(".var")
            .join("app")
            .join("com.valvesoftware.Steam")
            .join(".local")
            .join("share")
            .join("Steam"),
    ]
}
```

Add platform-specific private `platform_steam_roots()` functions:

- Windows: registry root if available, plus `ProgramFiles(x86)\Steam`.
- Linux: `linux_steam_roots_from_home(home)` using `HOME`.
- Other platforms: empty list.

- [ ] **Step 2: Export root provider module**

Modify `src-tauri/crates/hmm-infra/src/steam_discovery/mod.rs`:

```rust
mod key_values;
mod library_manifest;
mod root_provider;

pub use library_manifest::{
    parse_app_manifest, parse_library_folders, SteamAppManifest, SteamLibraryFolder,
};
pub use root_provider::{PlatformSteamRootProvider, SteamRootProvider};
```

- [ ] **Step 3: Add Windows registry dependency**

Modify `src-tauri/crates/hmm-infra/Cargo.toml`:

```toml
[target.'cfg(windows)'.dependencies]
winreg = "0.55"
```

- [ ] **Step 4: Add root provider tests**

Add tests in `root_provider.rs`:

```rust
#[test]
fn steam_root_builds_linux_candidate_roots_from_home() {
    let roots = linux_steam_roots_from_home(std::path::Path::new("/home/deck"));

    assert_eq!(roots[0], std::path::PathBuf::from("/home/deck/.steam/steam"));
    assert!(roots
        .iter()
        .any(|root| root.ends_with(".var/app/com.valvesoftware.Steam/.local/share/Steam")));
}
```

- [ ] **Step 5: Run root tests**

Run:

```powershell
cargo test -p hmm-infra steam_root
```

Expected:

```text
test result: ok
```

- [ ] **Step 6: Commit root provider**

Run:

```powershell
git add src-tauri/crates/hmm-infra/Cargo.toml Cargo.lock src-tauri/crates/hmm-infra/src/steam_discovery/root_provider.rs
git commit -m "feat: 添加 Steam 根目录识别器"
```

## Task 6: Steam Discovery Service

**Files:**

- Modify: `src-tauri/crates/hmm-infra/src/game_discovery.rs`
- Modify: `src-tauri/crates/hmm-infra/src/lib.rs`
- Test: `cargo test -p hmm-infra steam_discovery`

- [ ] **Step 1: Replace no-op service with Steam service**

In `src-tauri/crates/hmm-infra/src/game_discovery.rs`, keep `NoopGameDiscoveryService` for tests if useful, and add:

```rust
use crate::steam_discovery::{
    parse_app_manifest, parse_library_folders, SteamRootProvider,
};
use hmm_ports::{
    GameCandidate, GameCandidateSource, GameDiscoveryError, GameDiscoveryRequest,
    GameDiscoveryService,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct SteamGameDiscoveryService {
    root_provider: Arc<dyn SteamRootProvider>,
}

impl SteamGameDiscoveryService {
    pub fn new(root_provider: Arc<dyn SteamRootProvider>) -> Self {
        Self { root_provider }
    }
}
```

Implement `scan_candidates`:

- If `request.steam_app_id` is `None`, return an empty vector.
- For each Steam root, read `steamapps/libraryfolders.vdf`.
- Parse library folders.
- For each folder containing target app id, read `steamapps/appmanifest_<app_id>.acf`.
- Parse install dir.
- Candidate root is `<library path>/steamapps/common/<install_dir>`.
- Deduplicate by normalized string form.
- Return `GameCandidateSource::Steam` and source label `Steam`。

- [ ] **Step 2: Add discovery tests using temp dirs**

Add tests in `game_discovery.rs`:

```rust
#[test]
fn steam_discovery_returns_candidate_from_app_manifest() {
    let temp = create_temp_steam_root();
    write_libraryfolders_with_mhw(&temp);
    write_mhw_manifest(&temp, "Monster Hunter World");

    let service = SteamGameDiscoveryService::new(Arc::new(FakeSteamRootProvider {
        roots: vec![temp.path().to_path_buf()],
    }));

    let candidates = service
        .scan_candidates(&GameDiscoveryRequest {
            game_id: GameId::mhw(),
            display_name: "Monster Hunter: World - Iceborne".to_owned(),
            steam_app_id: Some(582010),
        })
        .expect("scan");

    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].root_dir.ends_with("steamapps/common/Monster Hunter World"));
}
```

Also cover:

- Missing Steam root returns empty vector.
- Missing app manifest returns empty vector.
- `steam_app_id: None` returns empty vector.
- Duplicate libraries return one candidate.

- [ ] **Step 3: Export Steam service**

Modify `src-tauri/crates/hmm-infra/src/lib.rs`:

```rust
pub use game_discovery::{NoopGameDiscoveryService, SteamGameDiscoveryService};
pub use steam_discovery::PlatformSteamRootProvider;
```

`SteamGameDiscoveryService` 会通过 `crate::steam_discovery` 导入 parser 函数和 provider trait，所以 `steam_discovery/mod.rs` 必须已经按 Task 4 和 Task 5 导出 `parse_app_manifest`、`parse_library_folders` 和 `SteamRootProvider`。

- [ ] **Step 4: Run discovery tests**

Run:

```powershell
cargo test -p hmm-infra steam_discovery
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit discovery service**

Run:

```powershell
git add src-tauri/crates/hmm-infra/src/game_discovery.rs src-tauri/crates/hmm-infra/src/lib.rs
git commit -m "feat: 实现 Steam 游戏目录扫描"
```

## Task 7: App Layer Candidate Validation

**Files:**

- Modify: `src-tauri/crates/hmm-app/src/game_setup.rs`
- Test: `cargo test -p hmm-app scan_candidates`

- [ ] **Step 1: Add app-level scan output**

In `src-tauri/crates/hmm-app/src/game_setup.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameCandidateScan {
    pub game_id: GameId,
    pub candidates: Vec<GameSetupCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameSetupCandidate {
    pub candidate: GameCandidate,
    pub validation: GameDirectoryValidation,
}
```

- [ ] **Step 2: Change scan_candidates return type**

Replace:

```rust
pub fn scan_candidates(&self, game_id: GameId) -> Result<(), GameSetupServiceError>
```

with:

```rust
pub fn scan_candidates(&self, game_id: GameId) -> Result<GameCandidateScan, GameSetupServiceError>
```

Implementation rules:

- Require adapter.
- Build `GameDiscoveryRequest` from adapter game id, display name and `steam_app_id()`.
- Call discovery service.
- Validate each raw candidate with `validate_with_adapter`.
- Sort valid candidates before invalid candidates, then by confidence descending.
- Return `GameCandidateScan`.

- [ ] **Step 3: Update app tests**

Replace the old not-implemented scan test with tests for:

```rust
#[test]
fn scan_candidates_validates_discovered_directories() {
    // Fake discovery returns C:/MHW.
    // Fake adapter marks the directory valid.
    // Result contains one valid candidate with confidence from adapter validation.
}

#[test]
fn scan_candidates_sorts_valid_candidates_first() {
    // Fake discovery returns one invalid and one valid candidate.
    // Result order is valid candidate first.
}

#[test]
fn scan_candidates_maps_discovery_failure() {
    // Fake discovery returns ScanFailed("boom").
    // Service error maps to storage_failed until a dedicated scan_failed error code is added.
}
```

- [ ] **Step 4: Run app tests**

Run:

```powershell
cargo test -p hmm-app scan_candidates
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit app validation**

Run:

```powershell
git add src-tauri/crates/hmm-app/src/game_setup.rs
git commit -m "feat: 校验并排序游戏目录候选"
```

## Task 8: Tauri DTO and State Wiring

**Files:**

- Modify: `src-tauri/src/dto.rs`
- Modify: `src-tauri/src/game_setup_commands.rs`
- Modify: `src-tauri/src/state.rs`
- Test: `cargo test -p hmm-tauri`

- [ ] **Step 1: Add scan DTOs**

In `src-tauri/src/dto.rs`, add:

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCandidateScanDto {
    pub game_id: String,
    pub candidates: Vec<GameCandidateDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCandidateDto {
    pub game_id: String,
    pub display_name: String,
    pub directory: String,
    pub path_label: String,
    pub source: String,
    pub source_label: String,
    pub is_valid: bool,
    pub confidence: u8,
    pub evidence: Vec<GameDirectoryEvidenceDto>,
    pub errors: Vec<String>,
}
```

Add `candidate_scan_to_dto(scan: GameCandidateScan) -> GameCandidateScanDto`. Reuse `validation_to_dto` mapping logic for evidence and error codes.

- [ ] **Step 2: Change command return type**

In `src-tauri/src/game_setup_commands.rs`, change:

```rust
pub fn scan_game_candidates(game_id: String, state: State<'_, AppState>) -> Result<(), CommandErrorDto>
```

to:

```rust
pub fn scan_game_candidates(
    game_id: String,
    state: State<'_, AppState>,
) -> Result<GameCandidateScanDto, CommandErrorDto>
```

Map service output with `candidate_scan_to_dto`.

- [ ] **Step 3: Wire Steam service in AppState**

In `src-tauri/src/state.rs`, replace:

```rust
Arc::new(NoopGameDiscoveryService)
```

with:

```rust
Arc::new(SteamGameDiscoveryService::new(Arc::new(
    PlatformSteamRootProvider,
)))
```

Update imports from `hmm_infra`.

- [ ] **Step 4: Run Tauri tests**

Run:

```powershell
cargo test -p hmm-tauri
```

Expected:

```text
test result: ok
```

- [ ] **Step 5: Commit Tauri wiring**

Run:

```powershell
git add src-tauri/src/dto.rs src-tauri/src/game_setup_commands.rs src-tauri/src/state.rs
git commit -m "feat: 返回游戏目录候选 DTO"
```

## Task 9: Frontend Candidate State and API

**Files:**

- Modify: `src/features/game-setup/gameSetupTypes.ts`
- Modify: `src/features/game-setup/gameSetupApi.ts`
- Modify: `src/features/game-setup/gameSetupViewModel.ts`
- Modify: `src/features/game-setup/useGameSetup.ts`
- Modify: `src/shared/api/tauri.ts`
- Reference: `docs/superpowers/plans/2026-06-06-steam-library-discovery-implementation-frontend-appendix.md`
- Test: `cmd /c corepack pnpm run typecheck`

- [ ] **Step 1: Apply frontend candidate state appendix**

Follow `docs/superpowers/plans/2026-06-06-steam-library-discovery-implementation-frontend-appendix.md` Task A Step 1-4 exactly. This adds scan DTOs, updates the typed API, maps candidate DTOs, and stores candidates in `useGameSetup`.

- [ ] **Step 2: Run typecheck**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected:

```text
No TypeScript errors
```

- [ ] **Step 3: Commit frontend state**

Run:

```powershell
git add src/features/game-setup/gameSetupTypes.ts src/features/game-setup/gameSetupApi.ts src/features/game-setup/gameSetupViewModel.ts src/features/game-setup/useGameSetup.ts src/shared/api/tauri.ts
git commit -m "feat: 接入前端候选扫描状态"
```

## Task 10: Frontend Candidate List UI

**Files:**

- Create: `src/features/game-setup/GameDirectoryCandidateList.tsx`
- Create: `src/features/game-setup/GameDirectoryCandidateList.css`
- Modify: `src/features/dashboard/DashboardHeroCard.tsx`
- Modify: `src/features/dashboard/DashboardPage.tsx`
- Reference: `docs/superpowers/plans/2026-06-06-steam-library-discovery-implementation-frontend-appendix.md`
- Test: `cmd /c corepack pnpm run typecheck`
- Test: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1`

- [ ] **Step 1: Apply frontend candidate list appendix**

Follow `docs/superpowers/plans/2026-06-06-steam-library-discovery-implementation-frontend-appendix.md` Task B Step 1-3 exactly. This creates the candidate list component, adds styles, and wires candidates through Dashboard without making Dashboard inspect Steam-specific rules.

- [ ] **Step 2: Run frontend checks**

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

- [ ] **Step 3: Commit candidate UI**

Run:

```powershell
git add src/features/game-setup/GameDirectoryCandidateList.tsx src/features/game-setup/GameDirectoryCandidateList.css src/features/dashboard/DashboardHeroCard.tsx src/features/dashboard/DashboardPage.tsx
git commit -m "feat: 展示 Steam 游戏目录候选列表"
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
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected:

```text
Verify completed successfully
```

- [ ] **Step 5: Manual smoke test with fake Steam directory**

Run the app:

```powershell
cmd /c corepack pnpm run tauri:dev
```

Manual checks:

- Existing manual directory selection still works.
- If Steam is not installed or target game is not installed, UI shows no-candidate fallback.
- With a temporary fake Steam library wired through unit tests only, automated tests verify candidate construction.
- No test or smoke step writes to a real game directory.
- No test or smoke step reads a real player save directory.

## PR Handoff

- [ ] **Step 1: Confirm branch status**

Run:

```powershell
git status --short --branch --untracked-files=all
```

Expected:

```text
## codex/steam-library-discovery
```

Working tree must be clean except committed implementation history.

- [ ] **Step 2: Push branch**

Run:

```powershell
git push -u origin codex/steam-library-discovery
```

- [ ] **Step 3: Create PR**

Run:

```powershell
gh pr create --base main --head codex/steam-library-discovery --title "[codex] 实现 Steam 游戏目录候选扫描" --body "## 改动`n- 添加 Steam library 与 app manifest 扫描`n- 返回经过 MHW:I adapter 校验的游戏目录候选`n- 前端展示 Steam 候选列表并复用保存流程`n`n## 验证`n- cargo test --workspace`n- cargo check --workspace`n- cmd /c corepack pnpm run typecheck`n- cmd /c corepack pnpm run lint`n- cmd /c corepack pnpm run build`n- powershell -NoProfile -ExecutionPolicy Bypass -File .\\scripts\\verify.ps1`n`n## 风险`n- 本轮不做进程扫描和 Steam Deck 实机验证`n- 扫描结果不会自动保存，保存候选时仍重新校验`n- 不写入真实游戏目录或存档目录"
```

## Self-Review Checklist

- [ ] `582010` 只出现在 MHW:I adapter、测试数据或 Steam manifest 测试样本中。
- [ ] `libraryfolders.vdf` 和 `appmanifest_*.acf` 只出现在 infra discovery 或测试中。
- [ ] 前端没有硬编码 Steam 路径或 MHW:I 文件规则。
- [ ] `scan_game_candidates` 返回候选 DTO，不再返回 `void`。
- [ ] 选择候选仍调用 `save_game_directory`，没有绕过后端校验。
- [ ] 测试使用临时目录，不依赖真实 Steam 或真实游戏安装。
- [ ] 最终回复准确记录已执行和未执行的验证。
