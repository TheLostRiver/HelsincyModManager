# Save Directory Auto Discovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 MHW:I 存档目录自动发现、多 Steam 账户确认、后端 Steam 公开资料补全，并把确认后的存档目录持久化到当前 Profile 设置中。

**Architecture:** 后端负责 Steam root、userdata、SteamID64 转换、公开 profile XML 查询、候选排序、短期候选缓存和最终设置写入；前端只消费候选摘要、头像 URL、脱敏标签和 opaque id。Profile 页面提供确认入口，应用启动自检只做静默发现和居中偏上的悬浮提示；实际备份仍由 `SaveBackupService` 重新校验存档目录，自动发现不能绕过备份安全链路。

**Tech Stack:** Tauri 2、React 19、TypeScript、Rust workspace、SQLite、serde、quick-xml、reqwest blocking transport、临时目录测试、fake HTTP transport、PowerShell verify 脚本。

---

## Scope

本计划实现：

- MHW:I Steam userdata 存档目录发现：`<SteamRoot>/userdata/<account_id_32>/582010/remote/`。
- 已保存游戏目录和平台 Steam root 共同作为扫描线索。
- 唯一高置信候选自动写入当前 Profile 的 `save_directory`。
- 多候选按最近修改时间推荐，但必须由玩家确认。
- 多候选显示 Steam 昵称、头像、最近修改时间、脱敏账号标签、脱敏路径标签和证据。
- Steam profile XML 请求由 Rust 后端发起，前端不访问 Steam Community。
- 查询失败、超时、资料私密、XML 异常或头像 URL 不可信时降级展示，不阻塞选择。
- 应用启动后对 active profile 做后台自检，并通过居中偏上的悬浮 UI 提醒。

本计划不实现：

- 执行备份、恢复、安装、卸载、manifest 写入或保留策略清理。
- Steam Web API key、OAuth、登录态、cookie 或私有 profile 接口。
- 将候选缓存持久化到 SQLite。
- 在前端计算 SteamID64、解析 XML、拼接真实路径或提交真实存档路径。
- 单元测试依赖真实 MHW:I、真实 Steam userdata、真实网络或真实玩家存档。

## Safety Invariants

- DTO、日志、任务事件和诊断包不得包含完整本地路径、account id、完整 SteamID64、XML 原文或真实存档内容。
- `discoveryId` 和 `candidateId` 是后端生成的 opaque id；确认时前端只提交这两个 id。
- 后端确认候选前必须从短期缓存取回真实路径并重新验证。
- 已有有效 `save_directory` 不被自动覆盖。
- 已有失效 `save_directory` 返回 `existing_invalid`，提示用户重新检测或手动选择。
- `SaveBackupService` 保持备份前最终校验，自动发现写入设置不代表备份可跳过安全检查。

## Target File Structure

```text
src-tauri/
  Cargo.toml
  src/
    lib.rs
    state.rs
    save_directory_discovery_commands.rs
    save_directory_discovery_dto.rs
  crates/
    hmm-core/src/
      lib.rs
      save_directory.rs
    hmm-ports/src/
      lib.rs
      save_directory.rs
    hmm-games-mhw/src/
      lib.rs
      save_directory.rs
    hmm-app/src/
      lib.rs
      save_directory_discovery.rs
    hmm-infra/src/
      lib.rs
      save_directory_scanner.rs
      steam_profile.rs
      save_directory_pending_store.rs

src/
  App.tsx
  main.tsx
  features/profiles/
    ProfilePage.tsx
    SaveDirectoryPanel.tsx
    ProfileSaveDirectoryDiscoveryProvider.tsx
    ProfileSaveDirectoryFloatingNotice.tsx
    ProfileSaveDirectoryCandidateList.tsx
    ProfileSaveDirectoryDiscovery.css
    profileSaveDirectoryDiscoveryApi.ts
    profileSaveDirectoryDiscoveryTypes.ts
    profileSaveDirectoryDiscovery.test.mjs
    profileFrontendIntegration.test.mjs
    profileApi.test.mjs

docs/
  FRONTEND_BACKEND_CONTRACT.md
  TESTING.md
  SAVE_DIRECTORY_AUTO_DISCOVERY_DESIGN.md
```

职责锁定：

- `hmm-core`：纯领域枚举、结果摘要、SteamID64 转换 helper；不访问网络、数据库或真实文件系统。
- `hmm-ports`：扫描、规则、公开资料 client、候选缓存 traits。
- `hmm-games-mhw`：MHW:I app id、`582010/remote`、`SAVEDATA1000` 证据规则。
- `hmm-infra`：Steam userdata 文件系统扫描、profile XML HTTP transport/parser、短期内存候选缓存。
- `hmm-app`：发现流程编排、自动写入、确认写入、错误码和降级策略。
- `src-tauri/src/*`：DTO 映射、command 参数校验、`AppState` 服务装配。
- `features/profiles`：typed API、全局发现 provider、悬浮提示、Profile 页候选确认和自动检测按钮。

## Task 0: Preflight

**Files:**

- Read: `AGENTS.md`
- Read: `README.md`
- Read: `docs/ARCHITECTURE.md`
- Read: `docs/TESTING.md`
- Read: `SECURITY.md`
- Read: `docs/LOGGING.md`
- Read: `docs/SAVE_BACKUP_DESIGN.md`
- Read: `docs/SAVE_DIRECTORY_AUTO_DISCOVERY_DESIGN.md`
- Read: `src-tauri/src/state.rs`
- Read: `src/features/profiles/ProfilePage.tsx`

- [ ] **Step 1: Confirm branch and tree**

Run:

```powershell
git status --short --branch --untracked-files=all
```

Expected:

```text
## codex/<save-directory-implementation-branch>
```

The output must not include unrelated modified files. `.planning/` files are runtime context and must remain unstaged.

- [ ] **Step 2: Confirm current validation baseline**

Run:

```powershell
cargo test -p hmm-app --test save_backup
cargo test -p hmm-infra --test save_backup_writer
cmd /c corepack pnpm run test -- src/features/profiles/profileApi.test.mjs
```

Expected:

```text
test result: ok
```

The frontend test must pass and continue to prove profile APIs do not expose filesystem internals.

- [ ] **Step 3: Commit nothing**

Run:

```powershell
git status --short
```

Expected: only unrelated pre-existing files, or no output. Do not commit during preflight.

## Task 1: Core Models And Ports

**Files:**

- Create: `src-tauri/crates/hmm-core/src/save_directory.rs`
- Modify: `src-tauri/crates/hmm-core/src/lib.rs`
- Create: `src-tauri/crates/hmm-ports/src/save_directory.rs`
- Modify: `src-tauri/crates/hmm-ports/src/lib.rs`
- Test: `cargo test -p hmm-core save_directory`
- Test: `cargo test -p hmm-ports`

- [ ] **Step 1: Write failing core tests**

Create `src-tauri/crates/hmm-core/src/save_directory.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_id64_from_account_id32_uses_public_offset() {
        assert_eq!(steam_id64_from_account_id32(1), 76_561_197_960_265_729);
        assert_eq!(steam_id64_from_account_id32(u32::MAX), 76_561_202_255_232_023);
    }

    #[test]
    fn discovery_result_marks_recommended_candidate() {
        let result = SaveDirectoryDiscoveryResult {
            discovery_id: "discovery-a".to_owned(),
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            outcome: SaveDirectoryDiscoveryOutcome::ConfirmationRequired,
            recommended_candidate_id: Some("candidate-newer".to_owned()),
            candidates: vec![
                SaveDirectoryCandidateSummary {
                    candidate_id: "candidate-newer".to_owned(),
                    source: SaveDirectoryCandidateSource::SteamUserdata,
                    confidence: SaveDirectoryCandidateConfidence::High,
                    recommended: true,
                    account_name: Some("Hunter".to_owned()),
                    avatar_url: None,
                    account_label: "Steam user ****1234".to_owned(),
                    path_label: "Steam/userdata/<account>/582010/remote".to_owned(),
                    last_modified_at: Some(2_000),
                    evidence: vec!["Found MHW:I save file".to_owned()],
                },
            ],
            saved_settings: None,
            error_code: None,
        };

        assert_eq!(result.recommended_candidate_id.as_deref(), Some("candidate-newer"));
        assert!(result.candidates[0].recommended);
    }
}
```

Run:

```powershell
cargo test -p hmm-core save_directory
```

Expected: FAIL because the module and types do not exist.

- [ ] **Step 2: Implement core models**

Replace the test-only skeleton with this public shape:

```rust
use crate::{GameId, ProfileDirectorySelection, ProfileId};

pub const STEAM_ID64_ACCOUNT_ID_OFFSET: u64 = 76_561_197_960_265_728;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveDirectoryDiscoveryOutcome {
    AutoSaved,
    ConfirmationRequired,
    NotFound,
    ExistingValid,
    ExistingInvalid,
    ScanFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveDirectoryCandidateSource {
    SteamUserdata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SaveDirectoryCandidateConfidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveDirectoryCandidateSummary {
    pub candidate_id: String,
    pub source: SaveDirectoryCandidateSource,
    pub confidence: SaveDirectoryCandidateConfidence,
    pub recommended: bool,
    pub account_name: Option<String>,
    pub avatar_url: Option<String>,
    pub account_label: String,
    pub path_label: String,
    pub last_modified_at: Option<u128>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveDirectoryDiscoveryResult {
    pub discovery_id: String,
    pub game_id: GameId,
    pub profile_id: ProfileId,
    pub outcome: SaveDirectoryDiscoveryOutcome,
    pub recommended_candidate_id: Option<String>,
    pub candidates: Vec<SaveDirectoryCandidateSummary>,
    pub saved_settings: Option<ProfileDirectorySelection>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamAccountProfileSummary {
    pub account_name: Option<String>,
    pub avatar_url: Option<String>,
}

pub fn steam_id64_from_account_id32(account_id_32: u32) -> u64 {
    STEAM_ID64_ACCOUNT_ID_OFFSET + u64::from(account_id_32)
}
```

Modify `src-tauri/crates/hmm-core/src/lib.rs`:

```rust
mod save_directory;

pub use save_directory::{
    steam_id64_from_account_id32, SaveDirectoryCandidateConfidence,
    SaveDirectoryCandidateSource, SaveDirectoryCandidateSummary, SaveDirectoryDiscoveryOutcome,
    SaveDirectoryDiscoveryResult, SteamAccountProfileSummary, STEAM_ID64_ACCOUNT_ID_OFFSET,
};
```

- [ ] **Step 3: Run core tests**

Run:

```powershell
cargo test -p hmm-core save_directory
```

Expected:

```text
test result: ok
```

- [ ] **Step 4: Add ports**

Create `src-tauri/crates/hmm-ports/src/save_directory.rs`:

```rust
use anyhow::Result;
use hmm_core::{
    GameId, ProfileId, SaveDirectoryCandidateConfidence, SaveDirectoryCandidateSummary,
    SteamAccountProfileSummary,
};
use std::path::PathBuf;
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
        directory: &std::path::Path,
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
```

Modify `src-tauri/crates/hmm-ports/src/lib.rs`:

```rust
mod save_directory;

pub use save_directory::{
    GameSaveDirectoryRule, PendingSaveDirectoryCandidate, PendingSaveDirectoryCandidateStore,
    PendingSaveDirectoryDiscovery, ScannedSaveDirectoryCandidate, SteamAccountProfileClient,
    SteamUserdataScanRequest, SteamUserdataScanner,
};
```

- [ ] **Step 5: Run ports tests**

Run:

```powershell
cargo test -p hmm-ports
```

Expected:

```text
test result: ok
```

- [ ] **Step 6: Commit**

Run:

```powershell
git add src-tauri/crates/hmm-core/src/save_directory.rs src-tauri/crates/hmm-core/src/lib.rs src-tauri/crates/hmm-ports/src/save_directory.rs src-tauri/crates/hmm-ports/src/lib.rs
git commit -m "feat: 添加存档目录发现领域接口"
```

## Task 2: MHW Rule And Steam Userdata Scanner

**Files:**

- Create: `src-tauri/crates/hmm-games-mhw/src/save_directory.rs`
- Modify: `src-tauri/crates/hmm-games-mhw/src/lib.rs`
- Create: `src-tauri/crates/hmm-infra/src/save_directory_scanner.rs`
- Modify: `src-tauri/crates/hmm-infra/src/lib.rs`
- Test: `cargo test -p hmm-games-mhw save_directory`
- Test: `cargo test -p hmm-infra save_directory_scanner`

- [ ] **Step 1: Write failing MHW rule test**

Create `src-tauri/crates/hmm-games-mhw/src/save_directory.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use hmm_ports::GameSaveDirectoryRule;

    #[test]
    fn mhw_save_rule_points_to_steam_remote_directory() {
        let rule = MonsterHunterWorldSaveDirectoryRule;

        assert_eq!(rule.game_id().as_str(), "mhw");
        assert_eq!(rule.steam_app_id(), 582010);
        assert_eq!(rule.steam_remote_relative_path(), "582010/remote");
        assert_eq!(rule.known_save_file_names(), &["SAVEDATA1000"]);
        assert_eq!(rule.path_label(), "Steam/userdata/<account>/582010/remote");
    }
}
```

Run:

```powershell
cargo test -p hmm-games-mhw save_directory
```

Expected: FAIL because `MonsterHunterWorldSaveDirectoryRule` does not exist.

- [ ] **Step 2: Implement MHW rule**

Use this implementation:

```rust
use hmm_core::GameId;
use hmm_ports::GameSaveDirectoryRule;

pub struct MonsterHunterWorldSaveDirectoryRule;

impl GameSaveDirectoryRule for MonsterHunterWorldSaveDirectoryRule {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn steam_app_id(&self) -> u32 {
        582010
    }

    fn steam_remote_relative_path(&self) -> &'static str {
        "582010/remote"
    }

    fn known_save_file_names(&self) -> &'static [&'static str] {
        &["SAVEDATA1000"]
    }

    fn path_label(&self) -> &'static str {
        "Steam/userdata/<account>/582010/remote"
    }
}
```

Modify `src-tauri/crates/hmm-games-mhw/src/lib.rs`:

```rust
mod save_directory;

pub use save_directory::MonsterHunterWorldSaveDirectoryRule;
```

- [ ] **Step 3: Run MHW rule test**

Run:

```powershell
cargo test -p hmm-games-mhw save_directory
```

Expected:

```text
test result: ok
```

- [ ] **Step 4: Write failing scanner tests**

Create `src-tauri/crates/hmm-infra/src/save_directory_scanner.rs` with tests that build temp Steam roots:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::steam_discovery::SteamRootProvider;
    use hmm_core::{GameId, SaveDirectoryCandidateConfidence};
    use std::path::PathBuf;

    struct FakeSteamRootProvider {
        roots: Vec<PathBuf>,
    }

    impl SteamRootProvider for FakeSteamRootProvider {
        fn steam_roots(&self) -> Vec<PathBuf> {
            self.roots.clone()
        }
    }

    #[test]
    fn scanner_finds_high_confidence_mhw_save_with_known_file() {
        let temp = tempfile::tempdir().expect("temp dir");
        let remote = temp.path().join("userdata").join("1234").join("582010").join("remote");
        std::fs::create_dir_all(&remote).expect("create remote");
        std::fs::write(remote.join("SAVEDATA1000"), b"save").expect("write save");

        let scanner = SteamUserdataSaveDirectoryScanner::new(Box::new(FakeSteamRootProvider {
            roots: vec![temp.path().to_path_buf()],
        }));
        let candidates = scanner.scan_save_directories(&mhw_request()).expect("scan");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].account_id_32, 1234);
        assert_eq!(candidates[0].confidence, SaveDirectoryCandidateConfidence::High);
        assert_eq!(candidates[0].path_label, "Steam/userdata/<account>/582010/remote");
        assert!(candidates[0].evidence.iter().any(|item| item.contains("SAVEDATA1000")));
    }

    #[test]
    fn scanner_ignores_non_numeric_account_directories() {
        let temp = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(
            temp.path().join("userdata").join("not-an-id").join("582010").join("remote"),
        )
        .expect("create remote");

        let scanner = SteamUserdataSaveDirectoryScanner::new(Box::new(FakeSteamRootProvider {
            roots: vec![temp.path().to_path_buf()],
        }));

        assert!(scanner.scan_save_directories(&mhw_request()).expect("scan").is_empty());
    }

    #[test]
    fn scanner_uses_game_root_hint_to_derive_steam_root() {
        let temp = tempfile::tempdir().expect("temp dir");
        let game_root = temp
            .path()
            .join("steamapps")
            .join("common")
            .join("Monster Hunter World");
        std::fs::create_dir_all(&game_root).expect("create game root");
        let remote = temp.path().join("userdata").join("2222").join("582010").join("remote");
        std::fs::create_dir_all(&remote).expect("create remote");
        std::fs::write(remote.join("SAVEDATA1000"), b"save").expect("write save");

        let scanner = SteamUserdataSaveDirectoryScanner::new(Box::new(FakeSteamRootProvider {
            roots: Vec::new(),
        }));
        let mut request = mhw_request();
        request.game_root_hint = Some(game_root);

        let candidates = scanner.scan_save_directories(&request).expect("scan");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].account_id_32, 2222);
    }

    fn mhw_request() -> SteamUserdataScanRequest {
        SteamUserdataScanRequest {
            game_id: GameId::mhw(),
            game_root_hint: None,
            steam_app_id: 582010,
            remote_relative_path: "582010/remote".to_owned(),
            known_save_file_names: vec!["SAVEDATA1000".to_owned()],
            path_label: "Steam/userdata/<account>/582010/remote".to_owned(),
        }
    }
}
```

Run:

```powershell
cargo test -p hmm-infra save_directory_scanner
```

Expected: FAIL because scanner implementation is missing.

- [ ] **Step 5: Implement scanner**

Implement `SteamUserdataSaveDirectoryScanner` with these rules:

```rust
pub struct SteamUserdataSaveDirectoryScanner {
    root_provider: Box<dyn crate::steam_discovery::SteamRootProvider>,
}

impl SteamUserdataSaveDirectoryScanner {
    pub fn new(root_provider: Box<dyn crate::steam_discovery::SteamRootProvider>) -> Self {
        Self { root_provider }
    }
}
```

Implementation details:

- Build candidate Steam roots from `root_provider.steam_roots()` plus a root derived from `game_root_hint` when it matches `<steamRoot>/steamapps/common/<gameDir>`.
- De-duplicate roots using normalized path keys.
- For each root, inspect `userdata`.
- Only accept account directory names that parse as `u32`.
- Build remote path from `userdata/<account_id_32>/<remote_relative_path>`.
- Use `symlink_metadata` and reject symlinked account directories or remote directories.
- Require remote directory to exist, be a directory and be readable.
- `High` confidence requires at least one known save file such as `SAVEDATA1000`.
- `Medium` confidence may be a readable non-empty remote directory without a known save file.
- `Low` confidence may be a readable empty remote directory.
- `last_modified_at` should prefer known save file modified time, then remote directory modified time.
- `candidate_id` must be generated by the scanner without embedding path or account id in a readable form; use a stable hash of normalized root, account id and remote path.
- `account_label` must be masked, for example `Steam user ****1234`.
- `path_label` must use the rule label and never include a local drive or username.

- [ ] **Step 6: Export scanner**

Modify `src-tauri/crates/hmm-infra/src/lib.rs`:

```rust
mod save_directory_scanner;

pub use save_directory_scanner::SteamUserdataSaveDirectoryScanner;
```

- [ ] **Step 7: Run scanner tests**

Run:

```powershell
cargo test -p hmm-infra save_directory_scanner
```

Expected:

```text
test result: ok
```

- [ ] **Step 8: Commit**

Run:

```powershell
git add src-tauri/crates/hmm-games-mhw/src/save_directory.rs src-tauri/crates/hmm-games-mhw/src/lib.rs src-tauri/crates/hmm-infra/src/save_directory_scanner.rs src-tauri/crates/hmm-infra/src/lib.rs
git commit -m "feat: 扫描 MHW 存档 userdata 候选"
```

## Task 3: Steam Profile XML Client

**Files:**

- Modify: `Cargo.toml`
- Modify: `src-tauri/crates/hmm-infra/Cargo.toml`
- Create: `src-tauri/crates/hmm-infra/src/steam_profile.rs`
- Modify: `src-tauri/crates/hmm-infra/src/lib.rs`
- Test: `cargo test -p hmm-infra steam_profile`

- [ ] **Step 1: Add direct dependencies**

Modify workspace dependencies in `Cargo.toml`:

```toml
quick-xml = "0.39.4"
reqwest = { version = "0.13.4", default-features = false, features = ["blocking", "rustls-tls"] }
```

Modify `src-tauri/crates/hmm-infra/Cargo.toml`:

```toml
quick-xml.workspace = true
reqwest.workspace = true
```

- [ ] **Step 2: Write failing parser and client tests**

Create `src-tauri/crates/hmm-infra/src/steam_profile.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::steam_id64_from_account_id32;

    #[test]
    fn parser_reads_name_and_https_avatar_from_xml() {
        let steam_id64 = steam_id64_from_account_id32(42);
        let xml = format!(
            r#"<profile>
                <steamID64>{steam_id64}</steamID64>
                <steamID><![CDATA[Hunter Name]]></steamID>
                <avatarMedium><![CDATA[https://avatars.akamai.steamstatic.com/example_medium.jpg]]></avatarMedium>
              </profile>"#
        );

        let parsed = parse_steam_profile_xml(&xml, steam_id64).expect("parse profile");

        assert_eq!(parsed.account_name.as_deref(), Some("Hunter Name"));
        assert_eq!(
            parsed.avatar_url.as_deref(),
            Some("https://avatars.akamai.steamstatic.com/example_medium.jpg")
        );
    }

    #[test]
    fn parser_drops_untrusted_avatar_url() {
        let steam_id64 = steam_id64_from_account_id32(42);
        let xml = format!(
            r#"<profile>
                <steamID64>{steam_id64}</steamID64>
                <steamID>Hunter Name</steamID>
                <avatarFull>http://example.invalid/avatar.jpg</avatarFull>
              </profile>"#
        );

        let parsed = parse_steam_profile_xml(&xml, steam_id64).expect("parse profile");

        assert_eq!(parsed.account_name.as_deref(), Some("Hunter Name"));
        assert_eq!(parsed.avatar_url, None);
    }

    #[test]
    fn parser_rejects_mismatched_steam_id64() {
        let wrong_id64 = hmm_core::steam_id64_from_account_id32(1);
        let xml = format!(
            r#"<profile><steamID64>{wrong_id64}</steamID64><steamID>Wrong</steamID></profile>"#
        );

        let error = parse_steam_profile_xml(&xml, hmm_core::steam_id64_from_account_id32(2))
            .expect_err("mismatched profile must fail");

        assert!(error.to_string().contains("steam id mismatch"));
    }
}
```

Run:

```powershell
cargo test -p hmm-infra steam_profile
```

Expected: FAIL because parser and transport are missing.

- [ ] **Step 3: Implement parser and client boundary**

Implement:

```rust
use hmm_core::{steam_id64_from_account_id32, SteamAccountProfileSummary};
use hmm_ports::SteamAccountProfileClient;
use std::time::Duration;

pub struct SteamCommunityProfileClient {
    transport: Box<dyn SteamProfileHttpTransport>,
}

pub trait SteamProfileHttpTransport: Send + Sync {
    fn get_profile_xml(&self, steam_id64: u64, timeout: Duration) -> anyhow::Result<String>;
}
```

Rules:

- `SteamCommunityProfileClient::fetch_profile(account_id_32, timeout)` converts with `steam_id64_from_account_id32`.
- Request URL is `https://steamcommunity.com/profiles/<steam_id64>/?xml=1`.
- `ReqwestSteamProfileHttpTransport` uses blocking reqwest and the passed timeout.
- Parser uses `quick-xml` events and reads only `steamID64`, `steamID`, `avatarMedium`, `avatarFull`.
- If both `avatarMedium` and `avatarFull` exist, prefer `avatarMedium`.
- Avatar URL must be HTTPS and start with `https://avatars.akamai.steamstatic.com/` or `https://avatars.steamstatic.com/`.
- Parser returns an error for mismatched `steamID64`, malformed XML, or missing profile root.
- App service will catch errors and degrade candidate display, so parser errors must not include XML body.

- [ ] **Step 4: Export client**

Modify `src-tauri/crates/hmm-infra/src/lib.rs`:

```rust
mod steam_profile;

pub use steam_profile::{
    parse_steam_profile_xml, ReqwestSteamProfileHttpTransport, SteamCommunityProfileClient,
    SteamProfileHttpTransport,
};
```

- [ ] **Step 5: Run tests**

Run:

```powershell
cargo test -p hmm-infra steam_profile
```

Expected:

```text
test result: ok
```

- [ ] **Step 6: Commit**

Run:

```powershell
git add Cargo.toml src-tauri/crates/hmm-infra/Cargo.toml src-tauri/crates/hmm-infra/src/steam_profile.rs src-tauri/crates/hmm-infra/src/lib.rs Cargo.lock
git commit -m "feat: 补全 Steam 账号资料解析"
```

## Task 4: Pending Store And App Service

**Files:**

- Create: `src-tauri/crates/hmm-infra/src/save_directory_pending_store.rs`
- Modify: `src-tauri/crates/hmm-infra/src/lib.rs`
- Create: `src-tauri/crates/hmm-app/src/save_directory_discovery.rs`
- Modify: `src-tauri/crates/hmm-app/src/lib.rs`
- Test: `cargo test -p hmm-infra pending_save_directory`
- Test: `cargo test -p hmm-app --test save_directory_discovery`

- [ ] **Step 1: Write pending store tests**

Create tests in `src-tauri/crates/hmm-infra/src/save_directory_pending_store.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{GameId, ProfileId, SaveDirectoryCandidateConfidence, SaveDirectoryCandidateSource};
    use hmm_ports::{PendingSaveDirectoryCandidate, PendingSaveDirectoryDiscovery};
    use std::path::PathBuf;

    #[test]
    fn store_returns_candidate_before_expiry_and_removes_expired_entries() {
        let store = InMemoryPendingSaveDirectoryCandidateStore::default();
        store.put(PendingSaveDirectoryDiscovery {
            discovery_id: "discovery-a".to_owned(),
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            expires_at_unix_millis: 1_500,
            candidates: vec![candidate("candidate-a")],
        }).expect("put");

        assert!(store.get_candidate("discovery-a", "candidate-a", 1_000).expect("get").is_some());
        assert!(store.get_candidate("discovery-a", "candidate-a", 2_000).expect("expired").is_none());
    }

    fn candidate(candidate_id: &str) -> PendingSaveDirectoryCandidate {
        PendingSaveDirectoryCandidate {
            summary: hmm_core::SaveDirectoryCandidateSummary {
                candidate_id: candidate_id.to_owned(),
                source: SaveDirectoryCandidateSource::SteamUserdata,
                confidence: SaveDirectoryCandidateConfidence::High,
                recommended: true,
                account_name: None,
                avatar_url: None,
                account_label: "Steam user ****1234".to_owned(),
                path_label: "Steam/userdata/<account>/582010/remote".to_owned(),
                last_modified_at: Some(1_000),
                evidence: vec!["Found MHW:I save file".to_owned()],
            },
            account_id_32: 1234,
            directory: PathBuf::from("C:/Synthetic/Steam/userdata/1234/582010/remote"),
        }
    }
}
```

Run:

```powershell
cargo test -p hmm-infra pending_save_directory
```

Expected: FAIL because the store does not exist.

- [ ] **Step 2: Implement pending store**

Implement `InMemoryPendingSaveDirectoryCandidateStore` with a `Mutex<HashMap<String, PendingSaveDirectoryDiscovery>>`.

Rules:

- `put` replaces an existing discovery with the same id.
- `get_candidate` removes expired discoveries before lookup.
- `get_candidate` returns a cloned candidate.
- The store never logs paths or ids.

Export it from `hmm-infra/src/lib.rs`:

```rust
mod save_directory_pending_store;

pub use save_directory_pending_store::InMemoryPendingSaveDirectoryCandidateStore;
```

- [ ] **Step 3: Run pending store tests**

Run:

```powershell
cargo test -p hmm-infra pending_save_directory
```

Expected:

```text
test result: ok
```

- [ ] **Step 4: Write app service tests**

Create `src-tauri/crates/hmm-app/tests/save_directory_discovery.rs`.

Cover these test names:

```rust
#[test]
fn discovery_auto_saves_single_high_confidence_candidate() {}

#[test]
fn discovery_requires_confirmation_for_multiple_candidates_and_recommends_newest() {}

#[test]
fn discovery_does_not_overwrite_existing_valid_setting() {}

#[test]
fn discovery_reports_existing_invalid_setting_without_auto_overwrite() {}

#[test]
fn discovery_degrades_when_steam_profile_lookup_fails() {}

#[test]
fn confirm_candidate_revalidates_and_saves_selected_directory() {}

#[test]
fn confirm_candidate_rejects_expired_candidate() {}
```

Use fake repositories, fake scanner, fake profile client, fake pending store and fake clock. Use synthetic paths only, such as `C:/Synthetic/Steam/userdata/1234/582010/remote`.

Run:

```powershell
cargo test -p hmm-app --test save_directory_discovery
```

Expected: FAIL because `ProfileSaveDirectoryDiscoveryService` does not exist.

- [ ] **Step 5: Implement app service**

Create `src-tauri/crates/hmm-app/src/save_directory_discovery.rs` with:

```rust
pub struct DiscoverProfileSaveDirectoriesRequest {
    pub game_id: GameId,
    pub profile_id: ProfileId,
}

pub struct ConfirmProfileSaveDirectoryCandidateRequest {
    pub discovery_id: String,
    pub candidate_id: String,
}

pub struct ProfileSaveDirectoryDiscoveryService {
    // game config repository, profile repository, settings repository,
    // rules, scanner, profile client, pending store, clock
}
```

Behavior:

- Load game instance by `game_id`; missing game returns outcome `scan_failed` with error code `save_directory_discovery_game_unconfigured`.
- Load requested profile; missing profile returns stable error `save_directory_discovery_profile_missing`.
- Read existing profile save settings.
- If existing save directory exists and scanner `validate_save_directory` returns high or medium confidence, return `ExistingValid` and do not save.
- If existing save directory exists but validation fails, return `ExistingInvalid` with error code `save_directory_discovery_candidate_invalid` and do not save.
- Build scan request from the matching `GameSaveDirectoryRule` and the saved game root as `game_root_hint`.
- Scan candidates.
- If no candidates, return `NotFound`.
- Enrich candidates with Steam profile summaries only when more than one candidate exists.
- Sort candidates by confidence descending, `last_modified_at` descending, then masked account label ascending.
- If exactly one candidate exists and confidence is `High`, save it as `ProfileSaveSettings.save_directory`.
- If more than one candidate exists, mark the newest candidate as recommended, cache the pending discovery for 10 minutes and return `ConfirmationRequired`.
- If scanner fails, return `ScanFailed` with code `save_directory_discovery_scan_failed`.
- If profile lookup fails, keep candidate selectable with `account_name = None` and `avatar_url = None`.
- `confirm_candidate` fetches candidate from store, revalidates the directory, saves selected settings, and returns `AutoSaved`.

When saving settings:

```rust
ProfileDirectorySelection {
    mode: ProfileDirectoryMode::Custom,
    status: ProfileDirectoryStatus::Valid,
    directory: Some(candidate.directory.to_string_lossy().to_string()),
    path_label: Some(candidate.path_label.clone()),
    messages: vec!["已自动关联 MHW:I 存档目录".to_owned()],
}
```

Preserve existing backup directory, schedule and retention when settings already exist; otherwise use validator default backup directory, manual schedule and default retention.

- [ ] **Step 6: Export app service**

Modify `src-tauri/crates/hmm-app/src/lib.rs`:

```rust
mod save_directory_discovery;

pub use save_directory_discovery::{
    ConfirmProfileSaveDirectoryCandidateRequest, DiscoverProfileSaveDirectoriesRequest,
    ProfileSaveDirectoryDiscoveryService, SaveDirectoryDiscoveryError,
};
```

- [ ] **Step 7: Run app service tests**

Run:

```powershell
cargo test -p hmm-app --test save_directory_discovery
```

Expected:

```text
test result: ok
```

- [ ] **Step 8: Commit**

Run:

```powershell
git add src-tauri/crates/hmm-infra/src/save_directory_pending_store.rs src-tauri/crates/hmm-infra/src/lib.rs src-tauri/crates/hmm-app/src/save_directory_discovery.rs src-tauri/crates/hmm-app/src/lib.rs src-tauri/crates/hmm-app/tests/save_directory_discovery.rs
git commit -m "feat: 编排存档目录自动发现"
```

## Task 5: Tauri DTO, Commands, And AppState Wiring

**Files:**

- Create: `src-tauri/src/save_directory_discovery_dto.rs`
- Create: `src-tauri/src/save_directory_discovery_commands.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `cargo test -p hmm-tauri save_directory_discovery`
- Test: `cargo check --workspace`

- [ ] **Step 1: Write failing DTO tests**

Create `src-tauri/src/save_directory_discovery_dto.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use hmm_core::{
        GameId, ProfileId, SaveDirectoryCandidateConfidence, SaveDirectoryCandidateSource,
        SaveDirectoryCandidateSummary, SaveDirectoryDiscoveryOutcome, SaveDirectoryDiscoveryResult,
    };
    use serde_json::Value;

    #[test]
    fn dto_serializes_without_raw_paths_or_steam_ids() {
        let result = SaveDirectoryDiscoveryResult {
            discovery_id: "discovery-a".to_owned(),
            game_id: GameId::mhw(),
            profile_id: ProfileId::new("default"),
            outcome: SaveDirectoryDiscoveryOutcome::ConfirmationRequired,
            recommended_candidate_id: Some("candidate-a".to_owned()),
            candidates: vec![SaveDirectoryCandidateSummary {
                candidate_id: "candidate-a".to_owned(),
                source: SaveDirectoryCandidateSource::SteamUserdata,
                confidence: SaveDirectoryCandidateConfidence::High,
                recommended: true,
                account_name: Some("Hunter".to_owned()),
                avatar_url: Some("https://avatars.akamai.steamstatic.com/example_medium.jpg".to_owned()),
                account_label: "Steam user ****1234".to_owned(),
                path_label: "Steam/userdata/<account>/582010/remote".to_owned(),
                last_modified_at: Some(1_000),
                evidence: vec!["Found MHW:I save file".to_owned()],
            }],
            saved_settings: None,
            error_code: None,
        };

        let value: Value = serde_json::to_value(SaveDirectoryDiscoveryDto::from(result)).expect("json");
        let serialized = value.to_string();

        assert_eq!(value["outcome"], "confirmation_required");
        assert_eq!(value["candidates"][0]["accountName"], "Hunter");
        assert!(!serialized.contains("C:/"));
        assert!(!serialized.contains("7656119"));
        assert!(!serialized.contains("1234/582010"));
    }
}
```

Run:

```powershell
cargo test -p hmm-tauri save_directory_discovery
```

Expected: FAIL because DTOs are missing.

- [ ] **Step 2: Implement DTOs**

DTO shape:

```rust
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDirectoryDiscoveryDto {
    pub discovery_id: String,
    pub game_id: String,
    pub profile_id: String,
    pub outcome: String,
    pub recommended_candidate_id: Option<String>,
    pub candidates: Vec<SaveDirectoryCandidateDto>,
    pub saved_settings: Option<crate::dto::ProfileSaveSettingsDto>,
    pub error_code: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDirectoryCandidateDto {
    pub candidate_id: String,
    pub source: String,
    pub confidence: String,
    pub recommended: bool,
    pub account_name: Option<String>,
    pub avatar_url: Option<String>,
    pub account_label: String,
    pub path_label: String,
    pub last_modified_at: Option<u128>,
    pub evidence: Vec<String>,
}
```

Map enum strings exactly:

```text
auto_saved
confirmation_required
not_found
existing_valid
existing_invalid
scan_failed
steam_userdata
high
medium
low
```

- [ ] **Step 3: Implement commands**

Create `src-tauri/src/save_directory_discovery_commands.rs`:

```rust
#[tauri::command]
pub fn discover_profile_save_directories(
    game_id: String,
    profile_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<SaveDirectoryDiscoveryDto, CommandErrorDto> {
    // parse ids, call state.save_directory_discovery.discover(...), map DTO
}

#[tauri::command]
pub fn confirm_profile_save_directory_candidate(
    discovery_id: String,
    candidate_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<SaveDirectoryDiscoveryDto, CommandErrorDto> {
    // normalize non-empty ids, call state.save_directory_discovery.confirm_candidate(...), map DTO
}
```

Rules:

- `game_id` must parse through `GameId::parse`.
- `profile_id`, `discovery_id`, `candidate_id` must be trimmed and non-empty.
- Error DTO code must use the app service stable code.
- Error message must be generic, for example `save directory discovery failed`.
- Command tests must assert error messages do not include path separators.

- [ ] **Step 4: Wire AppState**

Modify `src-tauri/src/state.rs`:

- Add `pub save_directory_discovery: Arc<ProfileSaveDirectoryDiscoveryService>`.
- Reuse existing `game_config_repository`, `profile_repository`, `profile_save_settings_repository`, `profile_save_directory_validator`, `SystemClock`.
- Add `MonsterHunterWorldSaveDirectoryRule`.
- Add `SteamUserdataSaveDirectoryScanner::new(Box::new(PlatformSteamRootProvider))`.
- Add `SteamCommunityProfileClient::new(Box::new(ReqwestSteamProfileHttpTransport))`.
- Add `InMemoryPendingSaveDirectoryCandidateStore::default()`.

- [ ] **Step 5: Register commands**

Modify `src-tauri/src/lib.rs`:

```rust
mod save_directory_discovery_commands;
mod save_directory_discovery_dto;

use save_directory_discovery_commands::{
    confirm_profile_save_directory_candidate, discover_profile_save_directories,
};
```

Add both commands to `tauri::generate_handler!`.

- [ ] **Step 6: Run Tauri tests and check**

Run:

```powershell
cargo test -p hmm-tauri save_directory_discovery
cargo check --workspace
```

Expected:

```text
test result: ok
Finished `dev` profile
```

- [ ] **Step 7: Commit**

Run:

```powershell
git add src-tauri/src/save_directory_discovery_dto.rs src-tauri/src/save_directory_discovery_commands.rs src-tauri/src/state.rs src-tauri/src/lib.rs
git commit -m "feat: 暴露存档目录发现命令"
```

## Task 6: Frontend Typed API

**Files:**

- Create: `src/features/profiles/profileSaveDirectoryDiscoveryTypes.ts`
- Create: `src/features/profiles/profileSaveDirectoryDiscoveryApi.ts`
- Create: `src/features/profiles/profileSaveDirectoryDiscovery.test.mjs`
- Modify: `src/features/profiles/profileApi.test.mjs`
- Test: `cmd /c corepack pnpm run test -- src/features/profiles/profileSaveDirectoryDiscovery.test.mjs src/features/profiles/profileApi.test.mjs`
- Test: `cmd /c corepack pnpm run typecheck`

- [ ] **Step 1: Write failing frontend source test**

Create `src/features/profiles/profileSaveDirectoryDiscovery.test.mjs`:

```js
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("save directory discovery API uses opaque candidate ids without raw filesystem or steam ids", () => {
  assert.equal(existsSync("src/features/profiles/profileSaveDirectoryDiscoveryApi.ts"), true);
  assert.equal(existsSync("src/features/profiles/profileSaveDirectoryDiscoveryTypes.ts"), true);

  const api = readSource("src/features/profiles/profileSaveDirectoryDiscoveryApi.ts");
  const types = readSource("src/features/profiles/profileSaveDirectoryDiscoveryTypes.ts");

  assert.match(api, /invoke<SaveDirectoryDiscoveryDto>\("discover_profile_save_directories",\s*input\)/);
  assert.match(api, /invoke<SaveDirectoryDiscoveryDto>\("confirm_profile_save_directory_candidate"/);
  assert.match(api, /discoveryId:\s*input\.discoveryId/);
  assert.match(api, /candidateId:\s*input\.candidateId/);
  assert.doesNotMatch(api, /rawPath|fullPath|steamId64|accountId|xml|profileUrl/i);

  assert.match(types, /candidateId:\s*string/);
  assert.match(types, /discoveryId:\s*string/);
  assert.match(types, /accountName:\s*string\s*\|\s*null/);
  assert.match(types, /avatarUrl:\s*string\s*\|\s*null/);
  assert.match(types, /pathLabel:\s*string/);
  assert.doesNotMatch(types, /rawPath|fullPath|steamId64|accountId|xml|profileUrl/i);
});
```

Run:

```powershell
cmd /c corepack pnpm run test -- src/features/profiles/profileSaveDirectoryDiscovery.test.mjs
```

Expected: FAIL because API files are missing.

- [ ] **Step 2: Add TypeScript types**

Create `src/features/profiles/profileSaveDirectoryDiscoveryTypes.ts`:

```ts
import type { ProfileSaveSettingsDto } from "./profileSaveSettingsTypes";

export type SaveDirectoryDiscoveryOutcome =
  | "auto_saved"
  | "confirmation_required"
  | "not_found"
  | "existing_valid"
  | "existing_invalid"
  | "scan_failed";

export type SaveDirectoryCandidateDto = {
  candidateId: string;
  source: "steam_userdata";
  confidence: "high" | "medium" | "low";
  recommended: boolean;
  accountName: string | null;
  avatarUrl: string | null;
  accountLabel: string;
  pathLabel: string;
  lastModifiedAt: number | null;
  evidence: string[];
};

export type SaveDirectoryDiscoveryDto = {
  discoveryId: string;
  gameId: string;
  profileId: string;
  outcome: SaveDirectoryDiscoveryOutcome;
  recommendedCandidateId: string | null;
  candidates: SaveDirectoryCandidateDto[];
  savedSettings?: ProfileSaveSettingsDto | null;
  errorCode?: string | null;
};

export type DiscoverProfileSaveDirectoriesInput = {
  gameId: string;
  profileId: string;
};

export type ConfirmProfileSaveDirectoryCandidateInput = {
  discoveryId: string;
  candidateId: string;
};
```

- [ ] **Step 3: Add typed API**

Create `src/features/profiles/profileSaveDirectoryDiscoveryApi.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import type {
  ConfirmProfileSaveDirectoryCandidateInput,
  DiscoverProfileSaveDirectoriesInput,
  SaveDirectoryDiscoveryDto,
} from "./profileSaveDirectoryDiscoveryTypes";

export function discoverProfileSaveDirectories(
  input: DiscoverProfileSaveDirectoriesInput,
): Promise<SaveDirectoryDiscoveryDto> {
  return invoke<SaveDirectoryDiscoveryDto>("discover_profile_save_directories", input);
}

export function confirmProfileSaveDirectoryCandidate(
  input: ConfirmProfileSaveDirectoryCandidateInput,
): Promise<SaveDirectoryDiscoveryDto> {
  return invoke<SaveDirectoryDiscoveryDto>("confirm_profile_save_directory_candidate", {
    discoveryId: input.discoveryId,
    candidateId: input.candidateId,
  });
}
```

- [ ] **Step 4: Extend profile API boundary test**

Modify `src/features/profiles/profileApi.test.mjs` and add a test that reads the new API files. Assert:

```js
assert.doesNotMatch(source, /rawPath|fullPath|steamId64|accountId|xml|profileUrl/i);
assert.doesNotMatch(typesSource, /rawPath|fullPath|steamId64|accountId|xml|profileUrl/i);
```

- [ ] **Step 5: Run frontend tests**

Run:

```powershell
cmd /c corepack pnpm run test -- src/features/profiles/profileSaveDirectoryDiscovery.test.mjs src/features/profiles/profileApi.test.mjs
cmd /c corepack pnpm run typecheck
```

Expected:

```text
ok
No TypeScript errors
```

- [ ] **Step 6: Commit**

Run:

```powershell
git add src/features/profiles/profileSaveDirectoryDiscoveryTypes.ts src/features/profiles/profileSaveDirectoryDiscoveryApi.ts src/features/profiles/profileSaveDirectoryDiscovery.test.mjs src/features/profiles/profileApi.test.mjs
git commit -m "feat: 添加存档目录发现前端接口"
```

## Task 7: Profile UI, Floating Notice, And Startup Self-Check

**Files:**

- Create: `src/features/profiles/ProfileSaveDirectoryDiscoveryProvider.tsx`
- Create: `src/features/profiles/ProfileSaveDirectoryFloatingNotice.tsx`
- Create: `src/features/profiles/ProfileSaveDirectoryCandidateList.tsx`
- Create: `src/features/profiles/ProfileSaveDirectoryDiscovery.css`
- Modify: `src/App.tsx`
- Modify: `src/main.tsx`
- Modify: `src/features/profiles/ProfilePage.tsx`
- Modify: `src/features/profiles/SaveDirectoryPanel.tsx`
- Modify: `src/features/profiles/profileFrontendIntegration.test.mjs`
- Test: `cmd /c corepack pnpm run test -- src/features/profiles/profileFrontendIntegration.test.mjs`
- Test: `cmd /c corepack pnpm run typecheck`

- [ ] **Step 1: Write failing UI source tests**

Add tests to `src/features/profiles/profileFrontendIntegration.test.mjs`:

```js
test("profile save discovery uses a floating notice and candidate confirmation UI", () => {
  const app = readSource("src/App.tsx");
  const main = readSource("src/main.tsx");
  const page = readSource("src/features/profiles/ProfilePage.tsx");
  const panel = readSource("src/features/profiles/SaveDirectoryPanel.tsx");
  const notice = readSource("src/features/profiles/ProfileSaveDirectoryFloatingNotice.tsx");
  const candidates = readSource("src/features/profiles/ProfileSaveDirectoryCandidateList.tsx");
  const css = readSource("src/features/profiles/ProfileSaveDirectoryDiscovery.css");

  assert.match(app, /ProfileSaveDirectoryDiscoveryProvider/);
  assert.match(main, /ProfileSaveDirectoryDiscovery\.css/);
  assert.match(page, /ProfileSaveDirectoryCandidateList/);
  assert.match(panel, /自动检测/);
  assert.match(notice, /positioned by CSS/);
  assert.match(notice, /window\.setTimeout/);
  assert.match(notice, /AUTO_DISMISS_TIMEOUT_MS\s*=\s*6000/);
  assert.match(candidates, /accountName/);
  assert.match(candidates, /avatarUrl/);
  assert.match(candidates, /recommended/);
  assert.match(css, /\.profile-save-directory-floating-notice\s*\{[\s\S]*?position:\s*fixed/);
  assert.match(css, /\.profile-save-directory-floating-notice\s*\{[\s\S]*?top:\s*clamp\(72px,\s*14vh,\s*128px\)/);
  assert.match(css, /\.profile-save-directory-floating-notice\s*\{[\s\S]*?left:\s*50%/);
  assert.match(css, /\.profile-save-directory-floating-notice\s*\{[\s\S]*?transform:\s*translateX\(-50%\)/);
  assert.doesNotMatch(page + panel + notice + candidates, /steamId64|accountId|rawPath|fullPath|xml/i);
});
```

Run:

```powershell
cmd /c corepack pnpm run test -- src/features/profiles/profileFrontendIntegration.test.mjs
```

Expected: FAIL because new UI files and imports do not exist.

- [ ] **Step 2: Add discovery provider**

Create `ProfileSaveDirectoryDiscoveryProvider.tsx`:

- Consume `useActiveProfile()`.
- In Tauri runtime only, call `discoverProfileSaveDirectories({ gameId: "mhw", profileId })` once when active profile is ready.
- Store `latestDiscovery`, `isDiscovering`, `notice`, `runDiscovery`, `confirmCandidate`, `dismissNotice`.
- If outcome is `auto_saved`, show floating notice and refresh profile settings through a callback exposed to Profile page.
- If outcome is `confirmation_required`, show floating notice and keep `latestDiscovery` for Profile page candidate list.
- If outcome is `not_found`, `existing_invalid` or `scan_failed`, show floating notice with manual fallback language.
- If outcome is `existing_valid`, do not show notice.
- Use a runtime guard matching existing preview mode logic: `typeof window !== "undefined" && "__TAURI_INTERNALS__" in window`.

Provider value shape:

```ts
type ProfileSaveDirectoryDiscoveryContextValue = {
  latestDiscovery: SaveDirectoryDiscoveryDto | null;
  isDiscovering: boolean;
  notice: ProfileSaveDirectoryNotice | null;
  runDiscovery: (input: { gameId: string; profileId: string; reason: "startup" | "manual" }) => Promise<void>;
  confirmCandidate: (candidateId: string) => Promise<void>;
  dismissNotice: () => void;
};
```

- [ ] **Step 3: Add floating notice**

Create `ProfileSaveDirectoryFloatingNotice.tsx`.

Requirements:

- Include this source comment for the existing source test:

```tsx
// positioned by CSS
```

- Use `const AUTO_DISMISS_TIMEOUT_MS = 6000;`.
- Use `role="status"` and `aria-live="polite"`.
- Auto dismiss with `window.setTimeout`.
- Pause auto dismiss on hover and focus.
- Use action buttons:
  - `查看候选` when confirmation is required.
  - `重新检测` for retryable outcomes.
  - close icon button with `aria-label="关闭存档目录提示"`.
- Do not render raw path or Steam id fields.

- [ ] **Step 4: Add candidate list**

Create `ProfileSaveDirectoryCandidateList.tsx`.

Requirements:

- Render only when `latestDiscovery.outcome === "confirmation_required"`.
- Candidate card shows:
  - avatar image when `avatarUrl` exists.
  - fallback icon when no avatar.
  - `accountName` or `Steam 资料不可用`.
  - `accountLabel`.
  - `pathLabel`.
  - last modified relative label or `最近修改时间不可用`.
  - evidence list.
  - `推荐` badge when `recommended`.
  - button `选择此账户` that calls `confirmCandidate(candidate.candidateId)`.
- The component never accepts or displays a real path.

- [ ] **Step 5: Wire App**

Modify `src/App.tsx`:

```tsx
<ActiveProfileProvider>
  <ProfileSaveDirectoryDiscoveryProvider>
    <AppShell>
      <RouterOutlet />
    </AppShell>
  </ProfileSaveDirectoryDiscoveryProvider>
</ActiveProfileProvider>
```

Modify `src/main.tsx`:

```ts
import "./features/profiles/ProfileSaveDirectoryDiscovery.css";
```

- [ ] **Step 6: Wire Profile page and panel**

Modify `ProfilePage.tsx`:

- Consume `useProfileSaveDirectoryDiscovery()`.
- Pass `onAutoDetect={() => runDiscovery({ gameId: CURRENT_GAME_ID, profileId: selectedProfileId, reason: "manual" })}` to `SaveDirectoryPanel`.
- Render `ProfileSaveDirectoryCandidateList` below `SaveDirectoryPanel`.
- After `auto_saved` or confirm success, refresh settings by incrementing `settingsRefreshToken`.

Modify `SaveDirectoryPanel.tsx`:

- Add props:

```ts
onAutoDetect: () => void;
autoDetecting?: boolean;
hasDiscoveryCandidates?: boolean;
```

- Add a secondary or primary action next to “选择路径” for the save source card:

```tsx
<button type="button" className="profile-action-button" disabled={disabled || autoDetecting} onClick={onAutoDetect}>
  <Search size={14} />
  {autoDetecting ? "检测中..." : "自动检测"}
</button>
```

- Keep manual selection available.

- [ ] **Step 7: Add CSS**

Create `ProfileSaveDirectoryDiscovery.css` based on `GameSetupFloatingNotice.css`:

- `.profile-save-directory-floating-notice` uses `position: fixed`, `top: clamp(72px, 14vh, 128px)`, `left: 50%`, `transform: translateX(-50%)`, `z-index: 85`.
- Use existing tokens, border radius and panel shadow.
- Candidate list uses compact cards with avatar size `44px`, no nested cards.
- Mobile layout wraps actions below copy.

- [ ] **Step 8: Run frontend tests**

Run:

```powershell
cmd /c corepack pnpm run test -- src/features/profiles/profileFrontendIntegration.test.mjs
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
```

Expected:

```text
ok
No TypeScript errors
No ESLint errors
```

- [ ] **Step 9: Commit**

Run:

```powershell
git add src/App.tsx src/main.tsx src/features/profiles/ProfileSaveDirectoryDiscoveryProvider.tsx src/features/profiles/ProfileSaveDirectoryFloatingNotice.tsx src/features/profiles/ProfileSaveDirectoryCandidateList.tsx src/features/profiles/ProfileSaveDirectoryDiscovery.css src/features/profiles/ProfilePage.tsx src/features/profiles/SaveDirectoryPanel.tsx src/features/profiles/profileFrontendIntegration.test.mjs
git commit -m "feat: 接入存档目录发现确认 UI"
```

## Task 8: Contract Docs And Full Verification

**Files:**

- Modify: `docs/FRONTEND_BACKEND_CONTRACT.md`
- Modify: `docs/TESTING.md`
- Modify: `docs/SAVE_DIRECTORY_AUTO_DISCOVERY_DESIGN.md`
- Modify: `TODO.md`
- Test: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`

- [ ] **Step 1: Update command contract**

In `docs/FRONTEND_BACKEND_CONTRACT.md`, add commands:

```text
discover_profile_save_directories({ gameId, profileId })
confirm_profile_save_directory_candidate({ discoveryId, candidateId })
```

Document that DTOs contain only opaque ids, account display summaries, avatar URL, `pathLabel`, `accountLabel`, `lastModifiedAt`, `evidence`, `outcome` and optional saved settings. State that DTOs never contain complete local paths, account id, SteamID64, XML, profile URL or save file contents.

- [ ] **Step 2: Update testing guide**

In `docs/TESTING.md`, under 存档备份 or 游戏适配器, add focused tests:

```text
cargo test -p hmm-app --test save_directory_discovery
cargo test -p hmm-infra save_directory_scanner
cargo test -p hmm-infra steam_profile
cargo test -p hmm-tauri save_directory_discovery
cmd /c corepack pnpm run test -- src/features/profiles/profileSaveDirectoryDiscovery.test.mjs src/features/profiles/profileFrontendIntegration.test.mjs
```

State that all automated tests use temp/fake data and no real Steam account, game install or save directory.

- [ ] **Step 3: Update design and task index**

In `docs/SAVE_DIRECTORY_AUTO_DISCOVERY_DESIGN.md`, add a short implementation status section that links this plan and notes that the implementation follows the backend-owned Steam profile lookup boundary.

In `TODO.md`, under T8 independent documents, add this implementation plan path.

- [ ] **Step 4: Run focused verification**

Run:

```powershell
cargo test -p hmm-core save_directory
cargo test -p hmm-games-mhw save_directory
cargo test -p hmm-infra save_directory_scanner
cargo test -p hmm-infra steam_profile
cargo test -p hmm-infra pending_save_directory
cargo test -p hmm-app --test save_directory_discovery
cargo test -p hmm-tauri save_directory_discovery
cmd /c corepack pnpm run test -- src/features/profiles/profileSaveDirectoryDiscovery.test.mjs src/features/profiles/profileFrontendIntegration.test.mjs src/features/profiles/profileApi.test.mjs
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

Expected:

```text
test result: ok
ok
No TypeScript errors
No ESLint errors
vite build completed
```

- [ ] **Step 5: Run full verification**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected:

```text
Verify completed successfully
```

- [ ] **Step 6: Check sensitive strings**

Run:

```powershell
rg -n "rawPath|fullPath|steamId64|accountId|xml=1|C:\\Users|userdata\\\\[0-9]+|7656119" src src-tauri docs
```

Expected:

- Only code that constructs the backend Steam Community request may contain `xml=1`.
- Only the SteamID64 conversion helper or tests may contain the public offset pattern.
- No committed fixture contains real profile XML, real avatar URL, real local path, real account id or real save contents.

- [ ] **Step 7: Commit docs**

Run:

```powershell
git add docs/FRONTEND_BACKEND_CONTRACT.md docs/TESTING.md docs/SAVE_DIRECTORY_AUTO_DISCOVERY_DESIGN.md TODO.md
git commit -m "docs: 更新存档目录发现契约"
```

## Review Checklist

- [ ] The frontend never computes SteamID64.
- [ ] The frontend never fetches Steam Community XML.
- [ ] The frontend never sends real candidate paths during confirmation.
- [ ] The backend never logs XML body, full local path, account id or SteamID64.
- [ ] Unique high-confidence candidate auto-save is covered by app tests.
- [ ] Multiple candidates require explicit confirmation and recommend the newest candidate.
- [ ] Profile lookup failures degrade display without blocking candidate selection.
- [ ] Confirming a candidate revalidates the directory before saving.
- [ ] Startup self-check uses a floating UI at the upper center and auto-dismisses after a few seconds.
- [ ] Save backup execution still revalidates the saved directory.
- [ ] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1` passes before PR handoff.
