# MHW:I Armor Retarget Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **当前优先级（2026-07-16）：** Gate A 已标记为 `certified`，本计划现为唯一 P1 / Gate B 主线；
> AR1-AR5 的代码、自动化与受控 UI 已标记为 `implemented`，当前只执行 Gate B Sandbox 验收。执行顺序以
> [核心 Mod 生命周期优先级计划](CORE_MOD_LIFECYCLE_PRIORITY_PLAN.md) 为准；按 AR1-AR5 重组执行，
> AR5 的 target switch 必须复用 Gate A 的真正重装 contract，不得引入独立删除/复制旁路。

**Goal:** 在 Helsincy Mod Manager 中实现第一版 MHW:I armor-retarget：玩家为外观 Mod 选择官方套装目标后，系统在 staging 中生成路径级重定向产物，并把结果交给 `InstallPlan` / manifest / backup / rollback 链路安装。

**Architecture:** 通用 replacement/retarget 模型放在 `hmm-core`，应用层只编排用例，MHW:I 的 catalog、路径解析和 slot 改写规则全部收敛到 `hmm-games-mhw`。原始导入包保持只读，retarget 只写 staging；冲突检测和真实游戏目录写入继续由安装链路负责。

**Tech Stack:** Rust workspace、Tauri 2、React + TypeScript、serde/serde_json、临时目录测试、PowerShell `scripts/verify.ps1`。

---

## Scope

本计划实现：

- MHW:I armor replacement catalog 的最小可运行版本。
- `ReplacementTarget`、`ReplacementBinding`、`ReplacementAnalysis`、`RetargetPlan`、`RetargetAction` 领域模型。
- MHW:I armor 路径解析：`nativePC/pl/f_equip/<slot>/arm/mod/<filename>`。
- 结构化 slot 段替换：只替换 `<slot>`，文件名和其他目录段原样保留。
- `m_equip` 显式识别并阻止第一版自动 retarget。
- catalog 加载时的 Unicode 归一化和搜索键规范化。
- staging materialize：从只读导入缓存复制到 staging 目标相对路径。
- manifest / audit 所需的 replacement binding 快照字段。
- 六个窄 Tauri command、前端 typed API 与 Mod 详情受控 UI，用于 catalog 查询、导入 Mod 分析、
  首次 retarget 安装，以及已安装 Mod 的真正重装 target switch 预览和任务启动。

本计划不实现：

- `.mod3` / `.mrl3` / `.tex` 二进制内部引用改写。
- 男体 `m_equip` retarget。
- 多源 slot 自动拆分。
- 武器、语音、NPC、随从外观 retarget。
- 直接生成可分发 zip。
- 使用或包装外部转换工具。

## Dependency Gate

armor-retarget 不再只以“安装 MVP 接口存在”为开工条件。开始任一运行时 Task 前，Core Mod
Lifecycle Gate A 必须达到 `certified`：安装/卸载纵向 acceptance、桌面 smoke、独立真正重装和
失败恢复证据全部通过。

```text
Gate A certified
  -> AR1 replacement model / ports / 最小 catalog [implemented]
  -> AR2 单 source f_equip parser / analyzer / RetargetPlan [implemented]
  -> AR3 staging materialize / InstallPlan / binding snapshot [implemented]
  -> AR4 target selection / preview / install UI [implemented]
  -> AR5 true reinstall target switch / uninstall [implemented]
  -> Gate B disposable Windows Sandbox acceptance [current]
```

旧计划中“Task 1-5 可在安装 MVP 前独立先做”的并行策略失效。这样做会再次扩大未被玩家闭环消费
的基础设施。Gate A 后仍可复用本计划的详细 RED/GREEN 步骤，但应按 AR1-AR5 重新分组提交，并
遵守以下范围：

- 第一条闭环只支持单 source `f_equip` 路径级 retarget。
- T9 只补 replacement binding snapshot；T10 只补 source/target/path-family 必要 preflight。
- 完整 catalog、本地化筛选、多 source、`m_equip`、武器/语音和高级 transformer 延后。
- target switch 不实现独立删除/复制旁路，必须生成新 plan 并调用真正重装链路。

## AR1 已实施基线（2026-07-16）

AR1 实际交付按重排后的窄边界实现，下面旧 Task 1-3 的大段代码草图只保留作历史追溯，不能再
逐字执行：旧草图中的 `ReplacementAnalysis` / `RetargetPlan` 属于 AR2，`PackageFileEntry` /
`StagingFileSystem` 属于 AR2/AR3，均未在 AR1 提前创建。

已落地：

- `hmm-core::replacement`：validated target/binding/source/kind/catalog-version identity、localized
  display map、opaque structured metadata、`ReplacementTarget`、`ReplacementBinding` 和
  `ReplacementCatalog` serde/invariant contract。
- `hmm-ports::replacement`：独立只读 `ReplacementCatalogProvider`，提供 catalog、stable-id find 与
  game-owned search；未修改目录 `GameAdapter`。
- `hmm-games-mhw::armor_retarget::catalog`：读取 `data/mhw-armor-targets.v1.json`，校验 schema、
  duplicate scoped internal id、`plNNN_VVVV` 与 metadata shape，执行 NFC/中点/NFKC 搜索规范化，
  并用精确 normalized terms 区分 Fatalis / Alatreon。
- 聚焦测试入口：`cargo test -p hmm-core --test replacement`、
  `cargo test -p hmm-ports --test replacement_catalog`、
  `cargo test -p hmm-games-mhw --test armor_catalog`。

AR1 没有 app/Tauri/frontend wiring，也没有 parser、analyzer、`RetargetPlan`、staging、InstallPlan 或
binding snapshot persistence。随后 AR2 从该公开类型/port 继续实现纯分析/计划，没有重新引入旧草图的宽 trait。

## AR2 已实施基线（2026-07-16）

AR2 按纯分析/计划边界落地；下面旧 Task 4-5 的代码草图只保留作历史追溯，实际公开 contract 以
当前源码和本节为准。

已落地：

- `hmm-core::retarget`：opaque `ReplacementSource`、`ReplacementAnalysis`、稳定 warning code、
  `RetargetAction` 与带 source/action/target/package/最终路径不变量的纯 `RetargetPlan`。
- `hmm-ports::replacement`：只携带 `PackageFileId` 与相对路径的 `ReplacementAsset`，以及窄
  analysis/plan request 和 `ReplacementAdapter`；没有 cache/sandbox `PathBuf` 或 staging port。
- `hmm-games-mhw::armor_retarget`：严格解析 `/`/`\\`，识别并阻断 `m_equip`/混合/多 source，
  只对结构化 `f_equip` slot 段生成目标相对路径；普通非 Armor 包返回不适用 warning。
- 聚焦测试入口：`cargo test -p hmm-core --test replacement_analysis`、
  `cargo test -p hmm-ports --test replacement_adapter`、
  `cargo test -p hmm-games-mhw --test armor_retarget`。

AR2 没有 app/infra/Tauri/frontend wiring，没有 materialize staging，也没有修改 InstallPlan/manifest。
AR3 必须消费这些纯 action，并保持原始导入包只读和 staging containment。

## AR3 已实施基线（2026-07-16）

AR3 按 app/ports/infra/core 的窄边界落地；下面旧 Task 6-9 的 `PathBuf`、单文件
`StagingFileSystem` 和展示型 snapshot 草图只保留作历史追溯，实际公开 contract 以当前源码和本节
为准。

已落地：

- `hmm-core`：validated `ReplacementBindingSnapshot`，带 Mod/profile/optional revision 归属、稳定
  source/target identity、path-family 与 retarget kind；`InstallPlan`、`InstallManifest` 和真正重装
  recovery transaction 默认兼容旧 JSON，并原子维护 snapshot 集合。
- `hmm-ports::staging`：batch `RetargetStagingMaterializer` 与只含原 `PackageFileId`、最终
  `InstallTargetPath` 的 `RetargetStagingFile`；错误使用稳定分类，不暴露 filesystem root。
- `hmm-app::replacement`：编排 adapter、snapshot、final-target InstallPlan 与 materialize；首次安装和
  真正重装在 source read、game write 和 manifest save 前校验 profile/revision ownership，plan/token
  hash 纳入 snapshot facts。
- `hmm-infra::staging`：在 sibling `.partial` 中完成受控 batch 写入，校验 containment、链接逃逸和
  大小写不敏感碰撞，完整成功后 rename 发布，失败清理；映射 source reader 保留原
  `PackageFileId` provenance。
- 聚焦测试入口：`cargo test -p hmm-core --test replacement_install`、
  `cargo test -p hmm-app --test replacement_service`、
  `cargo test -p hmm-infra --test retarget_staging`，以及完整 `hmm-app` / `hmm-infra` 回归。

AR3 没有 Tauri/AppState/frontend wiring，也没有开放已安装 Mod 的 target switch。AR4 只补 typed
contract 与最小受控 UI；真正重装 target switch、卸载闭环和 Gate B 验收仍属于 AR5。

AR4 正式 contract 以 `docs/FRONTEND_BACKEND_CONTRACT.md` 的“ARMOR_RETARGET AR4 契约”为准。
下方 Task 10/11 是 AR3 前历史草图，其中前端提交 `packageId`、`sourceAsset` 或完整 binding 的形状
已经废弃，不得照抄。正式实现从 `modId` 解析当前 display revision，由后端重建 source/binding；
并包含 `start_retarget_install_task` 的首次安装闭环与 installed-state fail-closed 门禁。

## AR4 已实施基线（2026-07-16）

AR4 按 Tauri thin shell、app workflow 与 feature-local frontend 边界落地；下方 Task 10/11 的旧
`packageId`/完整 binding 草图只保留作历史追溯，实际公开 contract 以当前源码和
`docs/FRONTEND_BACKEND_CONTRACT.md` 为准。

已落地：

- `list_replacement_targets`、`analyze_imported_mod_replacement`、
  `preview_initial_retarget_install`、`start_retarget_install_task` 四个 command；请求只接受
  game/Mod/profile/target/layer identity，拒绝未知字段，不接受 package/revision/source/path。
- 后端从当前 display revision 重建 package/source/binding/RetargetPlan/staging/InstallPlan；分析和
  materialize 在写锁外，runner 在锁内重新执行 profile recovery admission 与 `not_installed` 校验。
- 首次安装复用既有 install task、game/profile 写锁、Audit Log、backup、manifest、rollback/recovery；
  成功和失败/取消路径都会清理受控 staging。
- `src/features/replacements/` 提供 typed API、taskId/phase 状态机和目标面板；主入口位于
  `Mod 管理 -> Mod 详情 -> 替换目标` Tab，右键“MOD 文件修改”直达同一面板。
- UI 对 missing profile、installed、cleanup/rollback/repair/unknown、listener failure 和 blocking
  conflicts fail closed；完成闩锁跨 Tab 保持，旧 preview promise 不能覆盖新 target。
- 聚焦验证入口：`cargo test -p hmm-tauri replacement_dto_tests`、
  `cargo test -p hmm-tauri replacement_commands`、`cargo test -p hmm-app --test replacement_service`、
  `node --test src/features/replacements/*.test.mjs` 和完整 frontend test/typecheck/lint/build。
- 2026-07-16 disposable Windows Sandbox 人工纵向验收已通过：单 source 分析、跨槽位 target 选择、
  1 动作/0 阻断冲突预览、首次 retarget 安装、精确 target 字节与重启后 installed fail-closed 均符合预期。

AR4 没有实现已安装 Mod target switch、retarget-aware 卸载扩展或 Gate B 认证；这些仍属于 AR5。

## AR5 已实施基线（2026-07-16）

AR5 只扩展已安装 Mod 的同 revision target switch，并继续复用 Gate A 的真正重装事务；没有新增独立
删除、复制或原地改名路径。

已落地：

- `preview_retarget_reinstall` 与 `start_retarget_reinstall_task` 两个窄 command；请求只包含
  game/profile/Mod/target/layer identity，start 额外携带 preview `planToken`。installed revision 从
  manifest 解析，不接受前端 revision/package/source/binding/staging/path，也不会隐式升级版本。
- 同 revision 只在 persisted/candidate binding 证明同一 Mod/profile/source/path-family lineage 且 target
  确实变化时授权；普通相同 revision 重装和当前 target 重选继续 fail closed。
- 每次 target-switch materialize 使用独立 operation UUID 和 RAII cleanup；preview、取消、锁失败、
  commit 成功或失败均不把 staging 当作事实来源。
- commit 复用真正重装的 game/profile 写锁、plan token revalidation、backup、manifest entry-set replacement、
  rollback/recovery 与 `reinstall_mod` Audit Log；target switch 只新增稳定 `target_id` 审计字段。
- temp fixture 已证明旧 target 移除、新 target 安装、重启后 binding 恢复、最终 manifest 卸载恢复首次
  Armor 安装前逐字节 baseline。
- `src/features/replacements/` 已在 installed 状态开放 target-switch preview/confirm，展示四类 target 计数，
  严格按 taskId 消费事件；只在 queued/plan/preflight 安全阶段显示取消入口，commit/rollback 后隐藏。
- `analyze_imported_mod_replacement` 可接收可选 `profileId`，并从可信 manifest 只返回可选稳定
  `installedTargetId`；binding 歧义或读取失败时 fail closed，不返回 revision、binding、路径、staging
  或 manifest 内容。
- replacement Tab 在重启后选中并标记“当前已安装” target，同时禁用该 radio 和预览入口；选择其他
  target 后才进入真正重装切换预览。

首个 AR5 Sandbox artifact 已完成首次 retarget -> switch target -> restart -> uninstall -> exact
pre-Armor baseline 文件闭环，但第二次重启暴露出当前 target 未呈现缺陷。上述窄契约/UI 修复完成后，
必须重新构建最终 artifact 并在全新 disposable Windows Sandbox 重验；该证据成立前 Gate B 仍未
`certified`。

## Target File Structure（AR1-AR5 整体目标；已实施情况以上述基线为准）

```text
docs/
  ARMOR_RETARGET_DESIGN.md
  ARMOR_RETARGET_REVIEW.md
  ARMOR_RETARGET_IMPLEMENTATION.md

src-tauri/crates/hmm-core/src/
  game.rs
  replacement.rs
  retarget.rs
  lib.rs

src-tauri/crates/hmm-ports/src/
  game_setup.rs
  replacement.rs
  lib.rs

src-tauri/crates/hmm-app/src/
  game_setup.rs
  replacement.rs
  lib.rs

src-tauri/crates/hmm-games-mhw/src/
  armor_retarget/
    catalog.rs
    path.rs
    retarget.rs
    mod.rs
  lib.rs

src-tauri/crates/hmm-games-mhw/data/
  mhw-armor-targets.v1.json

src-tauri/crates/hmm-infra/src/
  staging.rs
  lib.rs

src-tauri/src/
  dto.rs
  replacement_commands.rs
  lib.rs
  state.rs

src/features/replacements/
  replacementApi.ts
  replacementTypes.ts
  replacementWorkflow.ts
  ReplacementTargetPanel.tsx
```

职责边界：

- `hmm-core/src/replacement.rs` 与 `retarget.rs`：AR1 定义游戏无关 identity/target/binding/catalog；AR2 已添加纯 analysis/plan。
- `hmm-ports/src/replacement.rs`：AR1 声明 catalog 查询；AR2 已添加纯 analysis/plan adapter；AR3 已在独立 `staging` 模块添加 batch materialize 端口。
- `hmm-games-mhw/src/armor_retarget/*`：AR1 已实现 catalog/Unicode；AR2 已实现 armor 路径解析、single-source analysis 和 slot 段替换。
- `hmm-app/src/replacement.rs`：AR3 已编排 catalog 查询、包分析、plan 生成、snapshot、InstallPlan 和 staging materialize。
- `hmm-infra/src/staging.rs`：AR3 已在受控 root 中 batch materialize 并原子发布，不触碰游戏目录。
- `src-tauri/src/replacement_commands.rs`：AR4 已添加 Tauri command 薄边界，只做 DTO 转换和调用应用层服务。
- `src/features/replacements/*`：AR4 已添加前端 typed API、task 状态机和受控目标面板，不拼接路径，不改写 slot 字符串。

## Task 0: Preflight

**Files:**

- Read: `AGENTS.md`
- Read: `README.md`
- Read: `docs/ARCHITECTURE.md`
- Read: `docs/ARMOR_RETARGET_DESIGN.md`
- Read: `docs/ARMOR_RETARGET_REVIEW.md`
- Read: `docs/TESTING.md`
- Read: `docs/GOVERNANCE.md`
- Read: `SECURITY.md`
- Read: `CONTRIBUTING.md`

- [ ] **Step 1: Confirm branch and unrelated changes**

Run:

```powershell
git status --short --branch --untracked-files=all
```

Expected:

```text
current branch is visible
unrelated files are not modified by this task
```

Do not stage `.planning/`, `.plan-attestation`, generated cache, fake game directories, backup directories, real Mod packages or real save files.

- [ ] **Step 2: Confirm baseline**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected:

```text
Verification passed.
```

## Task 1: Core Replacement Domain Model（AR1 已实施；下列旧草图仅供追溯）

**Files:**

- Create: `src-tauri/crates/hmm-core/src/replacement.rs`
- Modify: `src-tauri/crates/hmm-core/src/lib.rs`
- Test: `cargo test -p hmm-core replacement`

- [ ] **Step 1: Add domain types**

Create `src-tauri/crates/hmm-core/src/replacement.rs`:

```rust
use crate::GameId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplacementError {
    #[error("replacement target id cannot be empty")]
    EmptyTargetId,
    #[error("replacement internal id cannot be empty")]
    EmptyInternalId,
    #[error("replacement metadata field missing: {0}")]
    MissingMetadata(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReplacementTargetId(String);

impl ReplacementTargetId {
    pub fn parse(value: impl Into<String>) -> Result<Self, ReplacementError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(ReplacementError::EmptyTargetId);
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizedText {
    pub zh_cn: String,
    pub en: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplacementTarget {
    pub id: ReplacementTargetId,
    pub game_id: GameId,
    pub target_type: String,
    pub display_name: LocalizedText,
    pub aliases: Vec<String>,
    pub internal_id: String,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReplacementBindingId(String);

impl ReplacementBindingId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplacementBinding {
    pub id: ReplacementBindingId,
    pub mod_id: String,
    pub profile_id: String,
    pub source_asset: String,
    pub target_id: ReplacementTargetId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplacementAnalysis {
    pub detected_targets: Vec<ReplacementTargetId>,
    pub source_slots: Vec<String>,
    pub source_path_families: Vec<String>,
    pub supported_retarget_kinds: Vec<String>,
    pub warnings: Vec<ReplacementWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplacementWarning {
    MaleArmorPathRejected,
    MultipleSourceSlots,
    UnsupportedPathFamily(String),
    NoArmorPathDetected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetargetPlan {
    pub binding: ReplacementBinding,
    pub actions: Vec<RetargetAction>,
    pub warnings: Vec<ReplacementWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetargetAction {
    pub source_relative_path: String,
    pub staged_relative_path: String,
    pub source_slot: String,
    pub target_slot: String,
    pub source_path_family: String,
    pub target_path_family: String,
}
```

- [ ] **Step 2: Add core tests**

Append tests to `replacement.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_target_id_rejects_empty_value() {
        let result = ReplacementTargetId::parse(" ");
        assert_eq!(result, Err(ReplacementError::EmptyTargetId));
    }

    #[test]
    fn replacement_target_id_keeps_project_stable_key() {
        let id = ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("id");
        assert_eq!(id.as_str(), "mhw:armor:fatalis-alpha");
    }

    #[test]
    fn retarget_action_records_path_family_and_slots() {
        let action = RetargetAction {
            source_relative_path: "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3".to_owned(),
            staged_relative_path: "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3".to_owned(),
            source_slot: "pl121_0000".to_owned(),
            target_slot: "pl129_0000".to_owned(),
            source_path_family: "pl/f_equip".to_owned(),
            target_path_family: "pl/f_equip".to_owned(),
        };

        assert_eq!(action.source_slot, "pl121_0000");
        assert_eq!(action.target_path_family, "pl/f_equip");
    }
}
```

- [ ] **Step 3: Export module**

Modify `src-tauri/crates/hmm-core/src/lib.rs`:

```rust
mod game;
mod replacement;

pub use replacement::{
    LocalizedText, ReplacementAnalysis, ReplacementBinding, ReplacementBindingId, ReplacementError,
    ReplacementTarget, ReplacementTargetId, ReplacementWarning, RetargetAction, RetargetPlan,
};
```

Keep existing `game` exports unchanged.

- [ ] **Step 4: Verify core model**

Run:

```powershell
cargo test -p hmm-core replacement
```

Expected:

```text
test result: ok
```

## Task 2: Replacement Ports（AR1 已实施；下列宽 trait 草图已由窄 port 取代）

**Files:**

- Create: `src-tauri/crates/hmm-ports/src/replacement.rs`
- Modify: `src-tauri/crates/hmm-ports/src/lib.rs`
- Test: `cargo test -p hmm-ports`

- [ ] **Step 1: Define package file input and adapter traits**

Create `src-tauri/crates/hmm-ports/src/replacement.rs`:

```rust
use hmm_core::{
    GameId, ReplacementAnalysis, ReplacementBinding, ReplacementTarget, ReplacementTargetId,
    RetargetPlan,
};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplacementAdapterError {
    #[error("unsupported replacement target")]
    UnsupportedReplacementTarget,
    #[error("unrecognized source slot")]
    UnrecognizedSourceSlot,
    #[error("ambiguous source slot")]
    AmbiguousSourceSlot,
    #[error("unsafe retarget path")]
    UnsafeRetargetPath,
    #[error("target catalog missing")]
    TargetCatalogMissing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageFileEntry {
    pub relative_path: String,
    pub cache_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementAnalysisRequest {
    pub game_id: GameId,
    pub files: Vec<PackageFileEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetargetPlanRequest {
    pub game_id: GameId,
    pub binding: ReplacementBinding,
    pub files: Vec<PackageFileEntry>,
}

pub type ReplacementAdapterResult<T> = Result<T, ReplacementAdapterError>;

pub trait ReplacementAdapter: Send + Sync {
    fn game_id(&self) -> GameId;
    fn replacement_catalog(&self) -> ReplacementAdapterResult<Vec<ReplacementTarget>>;
    fn find_replacement_target(
        &self,
        target_id: &ReplacementTargetId,
    ) -> ReplacementAdapterResult<ReplacementTarget>;
    fn analyze_replacement_assets(
        &self,
        request: ReplacementAnalysisRequest,
    ) -> ReplacementAdapterResult<ReplacementAnalysis>;
    fn build_retarget_plan(
        &self,
        request: RetargetPlanRequest,
    ) -> ReplacementAdapterResult<RetargetPlan>;
}

#[derive(Debug, Error)]
pub enum StagingFileSystemError {
    #[error("staging path is unsafe")]
    UnsafePath,
    #[error("staging copy failed: {0}")]
    CopyFailed(String),
}

pub trait StagingFileSystem: Send + Sync {
    fn copy_to_staging(
        &self,
        source: &Path,
        staged_relative_path: &str,
    ) -> Result<PathBuf, StagingFileSystemError>;
}
```

- [ ] **Step 2: Export ports**

Modify `src-tauri/crates/hmm-ports/src/lib.rs`:

```rust
mod game_setup;
mod replacement;

pub use replacement::{
    PackageFileEntry, ReplacementAdapter, ReplacementAdapterError, ReplacementAdapterResult,
    ReplacementAnalysisRequest, RetargetPlanRequest, StagingFileSystem, StagingFileSystemError,
};
```

Keep existing `game_setup` exports unchanged.

- [ ] **Step 3: Verify ports**

Run:

```powershell
cargo test -p hmm-ports
```

Expected:

```text
test result: ok
```

## Task 3: MHW Armor Catalog（AR1 已实施；实际数据改用 versioned JSON envelope）

**Files:**

- Create: `src-tauri/crates/hmm-games-mhw/src/armor_retarget/mod.rs`
- Create: `src-tauri/crates/hmm-games-mhw/src/armor_retarget/catalog.rs`
- Create: `src-tauri/crates/hmm-games-mhw/src/armor_retarget/catalog_data.rs`
- Modify: `src-tauri/crates/hmm-games-mhw/src/lib.rs`
- Test: `cargo test -p hmm-games-mhw armor_catalog`

- [ ] **Step 1: Add catalog seed data**

Create `catalog_data.rs` with a minimal seed that covers the risky cases:

```rust
pub const ARMOR_CATALOG_JSON: &str = r#"
[
  {
    "id": "mhw:armor:guardian-alpha",
    "display_name_zh_cn": "【守护者α】服装",
    "display_name_en": "Guardian Alpha",
    "aliases": ["守护者α", "Guardian Alpha"],
    "internal_id": "pl121_0000",
    "path_family": "pl/f_equip",
    "monster": "guardian",
    "rank": "high",
    "variant": "alpha"
  },
  {
    "id": "mhw:armor:fatalis-alpha",
    "display_name_zh_cn": "【精英‧龙α】服装",
    "display_name_en": "Fatalis Alpha +",
    "aliases": ["黑龙α", "Fatalis Alpha"],
    "internal_id": "pl129_0000",
    "path_family": "pl/f_equip",
    "monster": "fatalis",
    "rank": "master",
    "variant": "alpha"
  },
  {
    "id": "mhw:armor:fatalis-beta",
    "display_name_zh_cn": "【精英‧龙β】服装",
    "display_name_en": "Fatalis Beta +",
    "aliases": ["黑龙β", "Fatalis Beta"],
    "internal_id": "pl129_0010",
    "path_family": "pl/f_equip",
    "monster": "fatalis",
    "rank": "master",
    "variant": "beta"
  },
  {
    "id": "mhw:armor:alatreon-alpha",
    "display_name_zh_cn": "【精英·煌黑龙α】服装",
    "display_name_en": "Alatreon Alpha +",
    "aliases": ["煌黑龙α", "Alatreon Alpha"],
    "internal_id": "pl052_0000",
    "path_family": "pl/f_equip",
    "monster": "alatreon",
    "rank": "master",
    "variant": "alpha"
  }
]
"#;
```

This seed is not the final full catalog. It is enough to prove schema, Unicode normalization, target lookup and path retarget behavior. Full catalog expansion should be a separate data task after this implementation path is green.

- [ ] **Step 2: Implement catalog loader**

Create `catalog.rs`:

```rust
use crate::armor_retarget::catalog_data::ARMOR_CATALOG_JSON;
use hmm_core::{GameId, LocalizedText, ReplacementError, ReplacementTarget, ReplacementTargetId};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
struct RawArmorTarget {
    id: String,
    display_name_zh_cn: String,
    display_name_en: Option<String>,
    aliases: Vec<String>,
    internal_id: String,
    path_family: String,
    monster: String,
    rank: String,
    variant: String,
}

pub fn load_armor_catalog() -> Result<Vec<ReplacementTarget>, ReplacementError> {
    let raw: Vec<RawArmorTarget> =
        serde_json::from_str(ARMOR_CATALOG_JSON).expect("bundled armor catalog must be valid json");

    raw.into_iter()
        .map(|item| {
            let mut metadata = BTreeMap::new();
            metadata.insert("path_family".to_owned(), item.path_family);
            metadata.insert("monster".to_owned(), normalize_search_text(&item.monster));
            metadata.insert("rank".to_owned(), item.rank);
            metadata.insert("variant".to_owned(), item.variant);

            Ok(ReplacementTarget {
                id: ReplacementTargetId::parse(item.id)?,
                game_id: GameId::mhw(),
                target_type: "armor".to_owned(),
                display_name: LocalizedText {
                    zh_cn: normalize_display_text(&item.display_name_zh_cn),
                    en: item.display_name_en,
                },
                aliases: item
                    .aliases
                    .into_iter()
                    .map(|alias| normalize_display_text(&alias))
                    .collect(),
                internal_id: item.internal_id,
                metadata,
            })
        })
        .collect()
}

pub fn normalize_display_text(value: &str) -> String {
    value
        .replace('\u{2027}', "·")
        .replace('\u{00B7}', "·")
        .replace('\u{30FB}', "·")
        .replace('\u{FF65}', "·")
}

pub fn normalize_search_text(value: &str) -> String {
    normalize_display_text(value).to_lowercase()
}
```

- [ ] **Step 3: Add catalog tests**

Append tests in `catalog.rs`:

```rust
#[cfg(test)]
mod armor_catalog_tests {
    use super::*;

    #[test]
    fn armor_catalog_contains_project_stable_ids() {
        let catalog = load_armor_catalog().expect("catalog");
        assert!(catalog
            .iter()
            .any(|item| item.id.as_str() == "mhw:armor:fatalis-alpha"));
    }

    #[test]
    fn armor_catalog_normalizes_middle_dot_codepoints() {
        assert_eq!(normalize_display_text("精英‧龙"), "精英·龙");
        assert_eq!(normalize_display_text("精英・龙"), "精英·龙");
        assert_eq!(normalize_display_text("精英･龙"), "精英·龙");
    }

    #[test]
    fn armor_catalog_distinguishes_fatalis_and_alatreon_by_monster() {
        let catalog = load_armor_catalog().expect("catalog");
        let fatalis = catalog
            .iter()
            .find(|item| item.id.as_str() == "mhw:armor:fatalis-alpha")
            .expect("fatalis");
        let alatreon = catalog
            .iter()
            .find(|item| item.id.as_str() == "mhw:armor:alatreon-alpha")
            .expect("alatreon");

        assert_eq!(fatalis.metadata.get("monster").map(String::as_str), Some("fatalis"));
        assert_eq!(alatreon.metadata.get("monster").map(String::as_str), Some("alatreon"));
    }
}
```

- [ ] **Step 4: Wire module**

Create `armor_retarget/mod.rs`:

```rust
mod catalog;
mod catalog_data;

pub use catalog::{load_armor_catalog, normalize_display_text, normalize_search_text};
```

Modify `hmm-games-mhw/src/lib.rs`:

```rust
mod armor_retarget;

pub use armor_retarget::load_armor_catalog;
```

Keep existing adapter code intact.

- [ ] **Step 5: Verify catalog**

Run:

```powershell
cargo test -p hmm-games-mhw armor_catalog
```

Expected:

```text
test result: ok
```

## Task 4: MHW Armor Path Parser（AR2 已实施；下列旧草图仅供追溯）

**Files:**

- Create: `src-tauri/crates/hmm-games-mhw/src/armor_retarget/path.rs`
- Modify: `src-tauri/crates/hmm-games-mhw/src/armor_retarget/mod.rs`
- Test: `cargo test -p hmm-games-mhw armor_path`

- [ ] **Step 1: Implement structured parser**

Create `path.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ArmorPathError {
    #[error("not an armor path")]
    NotArmorPath,
    #[error("male armor path is not supported in first version")]
    MaleArmorRejected,
    #[error("invalid armor slot")]
    InvalidSlot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmorResourcePath {
    pub normalized_path: String,
    pub path_family: String,
    pub slot: String,
    pub filename: String,
}

impl ArmorResourcePath {
    pub fn parse(value: &str) -> Result<Self, ArmorPathError> {
        let normalized = value.replace('\\', "/");
        let parts = normalized.split('/').collect::<Vec<_>>();

        if parts.len() != 7 || parts[0] != "nativePC" || parts[1] != "pl" {
            return Err(ArmorPathError::NotArmorPath);
        }

        let equip_family = parts[2];
        if equip_family == "m_equip" {
            return Err(ArmorPathError::MaleArmorRejected);
        }
        if equip_family != "f_equip" {
            return Err(ArmorPathError::NotArmorPath);
        }
        if parts[4] != "arm" || parts[5] != "mod" {
            return Err(ArmorPathError::NotArmorPath);
        }
        if !is_valid_slot(parts[3]) {
            return Err(ArmorPathError::InvalidSlot);
        }

        Ok(Self {
            normalized_path: parts.join("/"),
            path_family: "pl/f_equip".to_owned(),
            slot: parts[3].to_owned(),
            filename: parts[6].to_owned(),
        })
    }

    pub fn retarget(&self, target_slot: &str) -> Result<String, ArmorPathError> {
        if !is_valid_slot(target_slot) {
            return Err(ArmorPathError::InvalidSlot);
        }

        Ok(format!(
            "nativePC/pl/f_equip/{target_slot}/arm/mod/{}",
            self.filename
        ))
    }
}

fn is_valid_slot(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[0] == b'p'
        && bytes[1] == b'l'
        && bytes[2..5].iter().all(u8::is_ascii_digit)
        && bytes[5] == b'_'
        && bytes[6..10].iter().all(u8::is_ascii_digit)
}
```

- [ ] **Step 2: Add parser tests**

Append tests to `path.rs`:

```rust
#[cfg(test)]
mod armor_path_tests {
    use super::*;

    #[test]
    fn armor_path_accepts_forward_slashes() {
        let parsed = ArmorResourcePath::parse(
            "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
        )
        .expect("path");

        assert_eq!(parsed.slot, "pl121_0000");
        assert_eq!(parsed.path_family, "pl/f_equip");
    }

    #[test]
    fn armor_path_accepts_backslashes() {
        let parsed = ArmorResourcePath::parse(
            r"nativePC\pl\f_equip\pl121_0000\arm\mod\f_body.mod3",
        )
        .expect("path");

        assert_eq!(
            parsed.normalized_path,
            "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3"
        );
    }

    #[test]
    fn armor_path_rejects_male_path_for_first_version() {
        let error = ArmorResourcePath::parse(
            "nativePC/pl/m_equip/pl121_0000/arm/mod/m_body.mod3",
        )
        .expect_err("male path should be rejected");

        assert_eq!(error, ArmorPathError::MaleArmorRejected);
    }

    #[test]
    fn retarget_changes_only_slot_segment() {
        let parsed = ArmorResourcePath::parse(
            "nativePC/pl/f_equip/pl121_0000/arm/mod/f_121_0000_extra.mod3",
        )
        .expect("path");

        let target = parsed.retarget("pl129_0000").expect("retarget");

        assert_eq!(
            target,
            "nativePC/pl/f_equip/pl129_0000/arm/mod/f_121_0000_extra.mod3"
        );
    }
}
```

- [ ] **Step 3: Export parser**

Modify `armor_retarget/mod.rs`:

```rust
mod path;

pub use path::{ArmorPathError, ArmorResourcePath};
```

Keep catalog exports.

- [ ] **Step 4: Verify parser**

Run:

```powershell
cargo test -p hmm-games-mhw armor_path
```

Expected:

```text
test result: ok
```

## Task 5: MHW Replacement Adapter（AR2 已实施；下列旧草图仅供追溯）

**Files:**

- Create: `src-tauri/crates/hmm-games-mhw/src/armor_retarget/retarget.rs`
- Modify: `src-tauri/crates/hmm-games-mhw/src/armor_retarget/mod.rs`
- Modify: `src-tauri/crates/hmm-games-mhw/src/lib.rs`
- Test: `cargo test -p hmm-games-mhw armor_retarget`

- [ ] **Step 1: Implement adapter**

Create `retarget.rs`:

```rust
use crate::armor_retarget::{load_armor_catalog, ArmorPathError, ArmorResourcePath};
use hmm_core::{
    GameId, ReplacementAnalysis, ReplacementBinding, ReplacementTarget, ReplacementTargetId,
    ReplacementWarning, RetargetAction, RetargetPlan,
};
use hmm_ports::{
    PackageFileEntry, ReplacementAdapter, ReplacementAdapterError, ReplacementAdapterResult,
    ReplacementAnalysisRequest, RetargetPlanRequest,
};

pub struct MhwArmorReplacementAdapter;

impl ReplacementAdapter for MhwArmorReplacementAdapter {
    fn game_id(&self) -> GameId {
        GameId::mhw()
    }

    fn replacement_catalog(&self) -> ReplacementAdapterResult<Vec<ReplacementTarget>> {
        load_armor_catalog().map_err(|_| ReplacementAdapterError::TargetCatalogMissing)
    }

    fn find_replacement_target(
        &self,
        target_id: &ReplacementTargetId,
    ) -> ReplacementAdapterResult<ReplacementTarget> {
        self.replacement_catalog()?
            .into_iter()
            .find(|target| &target.id == target_id)
            .ok_or(ReplacementAdapterError::TargetCatalogMissing)
    }

    fn analyze_replacement_assets(
        &self,
        request: ReplacementAnalysisRequest,
    ) -> ReplacementAdapterResult<ReplacementAnalysis> {
        let parsed = collect_armor_paths(&request.files);
        let warnings = warnings_for_paths(&parsed);
        let source_slots = parsed
            .iter()
            .filter_map(|item| item.as_ref().ok())
            .map(|item| item.slot.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let source_path_families = parsed
            .iter()
            .filter_map(|item| item.as_ref().ok())
            .map(|item| item.path_family.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        Ok(ReplacementAnalysis {
            detected_targets: Vec::new(),
            source_slots,
            source_path_families,
            supported_retarget_kinds: vec!["mhw_armor_path".to_owned()],
            warnings,
        })
    }

    fn build_retarget_plan(
        &self,
        request: RetargetPlanRequest,
    ) -> ReplacementAdapterResult<RetargetPlan> {
        let target = self.find_replacement_target(&request.binding.target_id)?;
        let target_slot = target.internal_id;
        let target_path_family = target
            .metadata
            .get("path_family")
            .cloned()
            .ok_or(ReplacementAdapterError::TargetCatalogMissing)?;

        if target_path_family != "pl/f_equip" {
            return Err(ReplacementAdapterError::UnsupportedReplacementTarget);
        }

        let parsed = request
            .files
            .iter()
            .map(|file| {
                ArmorResourcePath::parse(&file.relative_path)
                    .map(|path| (file.relative_path.clone(), path))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_path_error)?;

        let slots = parsed
            .iter()
            .map(|(_, path)| path.slot.clone())
            .collect::<std::collections::BTreeSet<_>>();

        if slots.len() != 1 {
            return Err(ReplacementAdapterError::AmbiguousSourceSlot);
        }

        let actions = parsed
            .into_iter()
            .map(|(source_relative_path, source_path)| {
                let staged_relative_path = source_path
                    .retarget(&target_slot)
                    .map_err(map_path_error)?;

                Ok(RetargetAction {
                    source_relative_path,
                    staged_relative_path,
                    source_slot: source_path.slot,
                    target_slot: target_slot.clone(),
                    source_path_family: source_path.path_family,
                    target_path_family: target_path_family.clone(),
                })
            })
            .collect::<ReplacementAdapterResult<Vec<_>>>()?;

        Ok(RetargetPlan {
            binding: request.binding,
            actions,
            warnings: Vec::new(),
        })
    }
}

fn collect_armor_paths(
    files: &[PackageFileEntry],
) -> Vec<Result<ArmorResourcePath, ArmorPathError>> {
    files
        .iter()
        .map(|file| ArmorResourcePath::parse(&file.relative_path))
        .collect()
}

fn warnings_for_paths(paths: &[Result<ArmorResourcePath, ArmorPathError>]) -> Vec<ReplacementWarning> {
    let mut warnings = Vec::new();
    if paths.iter().all(Result::is_err) {
        warnings.push(ReplacementWarning::NoArmorPathDetected);
    }
    if paths
        .iter()
        .any(|item| matches!(item, Err(ArmorPathError::MaleArmorRejected)))
    {
        warnings.push(ReplacementWarning::MaleArmorPathRejected);
    }
    warnings
}

fn map_path_error(error: ArmorPathError) -> ReplacementAdapterError {
    match error {
        ArmorPathError::MaleArmorRejected => ReplacementAdapterError::AmbiguousSourceSlot,
        ArmorPathError::InvalidSlot => ReplacementAdapterError::UnrecognizedSourceSlot,
        ArmorPathError::NotArmorPath => ReplacementAdapterError::UnrecognizedSourceSlot,
    }
}
```

- [ ] **Step 2: Add adapter tests**

Append tests to `retarget.rs`:

```rust
#[cfg(test)]
mod armor_retarget_tests {
    use super::*;
    use hmm_core::{ReplacementBinding, ReplacementBindingId, ReplacementTargetId};
    use std::path::PathBuf;

    fn entry(path: &str) -> PackageFileEntry {
        PackageFileEntry {
            relative_path: path.to_owned(),
            cache_path: PathBuf::from("cache").join(path),
        }
    }

    fn binding(target_id: &str) -> ReplacementBinding {
        ReplacementBinding {
            id: ReplacementBindingId::new("binding-1"),
            mod_id: "mod-red-dress".to_owned(),
            profile_id: "default".to_owned(),
            source_asset: "pl121_0000".to_owned(),
            target_id: ReplacementTargetId::parse(target_id).expect("target id"),
        }
    }

    #[test]
    fn armor_retarget_plan_changes_only_slot_segment() {
        let adapter = MhwArmorReplacementAdapter;
        let plan = adapter
            .build_retarget_plan(RetargetPlanRequest {
                game_id: GameId::mhw(),
                binding: binding("mhw:armor:fatalis-alpha"),
                files: vec![entry(
                    "nativePC/pl/f_equip/pl121_0000/arm/mod/f_121_0000_extra.mod3",
                )],
            })
            .expect("plan");

        assert_eq!(
            plan.actions[0].staged_relative_path,
            "nativePC/pl/f_equip/pl129_0000/arm/mod/f_121_0000_extra.mod3"
        );
    }

    #[test]
    fn armor_retarget_rejects_unknown_target() {
        let adapter = MhwArmorReplacementAdapter;
        let error = adapter
            .build_retarget_plan(RetargetPlanRequest {
                game_id: GameId::mhw(),
                binding: binding("mhw:armor:not-real"),
                files: vec![entry(
                    "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3",
                )],
            })
            .expect_err("unknown target");

        assert_eq!(error, ReplacementAdapterError::TargetCatalogMissing);
    }

    #[test]
    fn armor_analysis_warns_for_male_path() {
        let adapter = MhwArmorReplacementAdapter;
        let analysis = adapter
            .analyze_replacement_assets(ReplacementAnalysisRequest {
                game_id: GameId::mhw(),
                files: vec![entry(
                    "nativePC/pl/m_equip/pl121_0000/arm/mod/m_body.mod3",
                )],
            })
            .expect("analysis");

        assert!(analysis
            .warnings
            .contains(&ReplacementWarning::MaleArmorPathRejected));
    }
}
```

- [ ] **Step 3: Export adapter**

Modify `armor_retarget/mod.rs`:

```rust
mod retarget;

pub use retarget::MhwArmorReplacementAdapter;
```

Modify `hmm-games-mhw/src/lib.rs`:

```rust
pub use armor_retarget::MhwArmorReplacementAdapter;
```

- [ ] **Step 4: Verify adapter**

Run:

```powershell
cargo test -p hmm-games-mhw armor_retarget
```

Expected:

```text
test result: ok
```

## Task 6: Application Replacement Service（AR3 已实施；下列旧草图仅供追溯）

**Files:**

- Create: `src-tauri/crates/hmm-app/src/replacement.rs`
- Modify: `src-tauri/crates/hmm-app/src/lib.rs`
- Test: `cargo test -p hmm-app replacement_service`

- [ ] **Step 1: Implement service**

Create `replacement.rs`:

```rust
use hmm_core::{
    GameId, ReplacementAnalysis, ReplacementBinding, ReplacementTarget, RetargetPlan,
};
use hmm_ports::{
    PackageFileEntry, ReplacementAdapter, ReplacementAdapterError, ReplacementAnalysisRequest,
    RetargetPlanRequest,
};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReplacementServiceError {
    #[error("unsupported game")]
    UnsupportedGame,
    #[error("adapter failed: {0}")]
    Adapter(#[from] ReplacementAdapterError),
}

pub struct ReplacementService {
    adapters: Vec<Arc<dyn ReplacementAdapter>>,
}

impl ReplacementService {
    pub fn new(adapters: Vec<Arc<dyn ReplacementAdapter>>) -> Self {
        Self { adapters }
    }

    pub fn catalog(&self, game_id: GameId) -> Result<Vec<ReplacementTarget>, ReplacementServiceError> {
        self.adapter_for(&game_id)?.replacement_catalog().map_err(Into::into)
    }

    pub fn analyze(
        &self,
        game_id: GameId,
        files: Vec<PackageFileEntry>,
    ) -> Result<ReplacementAnalysis, ReplacementServiceError> {
        self.adapter_for(&game_id)?
            .analyze_replacement_assets(ReplacementAnalysisRequest { game_id, files })
            .map_err(Into::into)
    }

    pub fn build_retarget_plan(
        &self,
        game_id: GameId,
        binding: ReplacementBinding,
        files: Vec<PackageFileEntry>,
    ) -> Result<RetargetPlan, ReplacementServiceError> {
        self.adapter_for(&game_id)?
            .build_retarget_plan(RetargetPlanRequest {
                game_id,
                binding,
                files,
            })
            .map_err(Into::into)
    }

    fn adapter_for(
        &self,
        game_id: &GameId,
    ) -> Result<Arc<dyn ReplacementAdapter>, ReplacementServiceError> {
        self.adapters
            .iter()
            .find(|adapter| adapter.game_id() == *game_id)
            .cloned()
            .ok_or(ReplacementServiceError::UnsupportedGame)
    }
}
```

- [ ] **Step 2: Add service tests**

Append tests:

```rust
#[cfg(test)]
mod replacement_service_tests {
    use super::*;
    use hmm_core::{LocalizedText, ReplacementTargetId};
    use std::collections::BTreeMap;

    struct FakeAdapter;

    impl ReplacementAdapter for FakeAdapter {
        fn game_id(&self) -> GameId {
            GameId::mhw()
        }

        fn replacement_catalog(&self) -> Result<Vec<ReplacementTarget>, ReplacementAdapterError> {
            Ok(vec![ReplacementTarget {
                id: ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("id"),
                game_id: GameId::mhw(),
                target_type: "armor".to_owned(),
                display_name: LocalizedText {
                    zh_cn: "黑龙α".to_owned(),
                    en: Some("Fatalis Alpha".to_owned()),
                },
                aliases: vec!["黑龙".to_owned()],
                internal_id: "pl129_0000".to_owned(),
                metadata: BTreeMap::new(),
            }])
        }

        fn find_replacement_target(
            &self,
            target_id: &ReplacementTargetId,
        ) -> Result<ReplacementTarget, ReplacementAdapterError> {
            self.replacement_catalog()?
                .into_iter()
                .find(|target| &target.id == target_id)
                .ok_or(ReplacementAdapterError::TargetCatalogMissing)
        }

        fn analyze_replacement_assets(
            &self,
            _request: ReplacementAnalysisRequest,
        ) -> Result<ReplacementAnalysis, ReplacementAdapterError> {
            Ok(ReplacementAnalysis {
                detected_targets: Vec::new(),
                source_slots: vec!["pl121_0000".to_owned()],
                source_path_families: vec!["pl/f_equip".to_owned()],
                supported_retarget_kinds: vec!["mhw_armor_path".to_owned()],
                warnings: Vec::new(),
            })
        }

        fn build_retarget_plan(
            &self,
            request: RetargetPlanRequest,
        ) -> Result<RetargetPlan, ReplacementAdapterError> {
            Ok(RetargetPlan {
                binding: request.binding,
                actions: Vec::new(),
                warnings: Vec::new(),
            })
        }
    }

    #[test]
    fn replacement_service_returns_catalog_for_supported_game() {
        let service = ReplacementService::new(vec![Arc::new(FakeAdapter)]);

        let catalog = service.catalog(GameId::mhw()).expect("catalog");

        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].internal_id, "pl129_0000");
    }
}
```

- [ ] **Step 3: Export service**

Modify `hmm-app/src/lib.rs`:

```rust
mod replacement;

pub use replacement::{ReplacementService, ReplacementServiceError};
```

Keep existing exports.

- [ ] **Step 4: Verify service**

Run:

```powershell
cargo test -p hmm-app replacement_service
```

Expected:

```text
test result: ok
```

## Task 7: Staging Materialize（AR3 已实施；下列单文件草图已由 batch port 取代）

**Files:**

- Create: `src-tauri/crates/hmm-infra/src/staging.rs`
- Modify: `src-tauri/crates/hmm-infra/src/lib.rs`
- Test: `cargo test -p hmm-infra staging`

- [ ] **Step 1: Implement staging file system**

Create `staging.rs`:

```rust
use hmm_ports::{StagingFileSystem, StagingFileSystemError};
use std::path::{Component, Path, PathBuf};

pub struct LocalStagingFileSystem {
    root: PathBuf,
}

impl LocalStagingFileSystem {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl StagingFileSystem for LocalStagingFileSystem {
    fn copy_to_staging(
        &self,
        source: &Path,
        staged_relative_path: &str,
    ) -> Result<PathBuf, StagingFileSystemError> {
        let relative = safe_relative_path(staged_relative_path)?;
        let target = self.root.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| StagingFileSystemError::CopyFailed(error.to_string()))?;
        }
        std::fs::copy(source, &target)
            .map_err(|error| StagingFileSystemError::CopyFailed(error.to_string()))?;
        Ok(target)
    }
}

fn safe_relative_path(value: &str) -> Result<PathBuf, StagingFileSystemError> {
    let normalized = value.replace('\\', "/");
    let path = Path::new(&normalized);
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(StagingFileSystemError::UnsafePath);
            }
        }
    }
    if result.as_os_str().is_empty() {
        return Err(StagingFileSystemError::UnsafePath);
    }
    Ok(result)
}
```

- [ ] **Step 2: Add staging tests**

Append tests:

```rust
#[cfg(test)]
mod staging_tests {
    use super::*;

    #[test]
    fn staging_copies_file_to_relative_target() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("source.bin");
        std::fs::write(&source, b"hello").expect("write");
        let staging_root = temp.path().join("staging");
        let fs = LocalStagingFileSystem::new(staging_root.clone());

        let target = fs
            .copy_to_staging(
                &source,
                "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3",
            )
            .expect("copy");

        assert!(target.starts_with(&staging_root));
        assert_eq!(std::fs::read(target).expect("read"), b"hello");
    }

    #[test]
    fn staging_rejects_parent_segments() {
        let temp = tempfile::tempdir().expect("temp");
        let source = temp.path().join("source.bin");
        std::fs::write(&source, b"hello").expect("write");
        let fs = LocalStagingFileSystem::new(temp.path().join("staging"));

        let error = fs
            .copy_to_staging(&source, "../escape.bin")
            .expect_err("unsafe path");

        assert!(matches!(error, StagingFileSystemError::UnsafePath));
    }
}
```

Add `tempfile` as a dev-dependency for `hmm-infra` if it is not already present.

- [ ] **Step 3: Export staging**

Modify `hmm-infra/src/lib.rs`:

```rust
mod staging;

pub use staging::LocalStagingFileSystem;
```

- [ ] **Step 4: Verify staging**

Run:

```powershell
cargo test -p hmm-infra staging
```

Expected:

```text
test result: ok
```

## Task 8: Retarget Materialize Use Case（AR3 已实施；实际 contract 见 AR3 基线）

**Files:**

- Modify: `src-tauri/crates/hmm-app/src/replacement.rs`
- Test: `cargo test -p hmm-app materialize_retarget`

- [ ] **Step 1: Add materialize request**

Extend `replacement.rs`:

```rust
use hmm_ports::{PackageFileEntry, StagingFileSystem, StagingFileSystemError};
use std::collections::BTreeMap;

#[derive(Debug, Error)]
pub enum RetargetMaterializeError {
    #[error("source path missing for retarget action")]
    SourceMissing,
    #[error("staging failed: {0}")]
    Staging(#[from] StagingFileSystemError),
}

pub struct MaterializeRetargetRequest {
    pub files: Vec<PackageFileEntry>,
    pub plan: RetargetPlan,
}

pub struct MaterializedRetarget {
    pub staged_paths: Vec<std::path::PathBuf>,
    pub plan: RetargetPlan,
}

impl ReplacementService {
    pub fn materialize_retarget(
        &self,
        staging: &dyn StagingFileSystem,
        request: MaterializeRetargetRequest,
    ) -> Result<MaterializedRetarget, RetargetMaterializeError> {
        let sources = request
            .files
            .iter()
            .map(|file| (file.relative_path.clone(), file.cache_path.clone()))
            .collect::<BTreeMap<_, _>>();

        let mut staged_paths = Vec::new();
        for action in &request.plan.actions {
            let source = sources
                .get(&action.source_relative_path)
                .ok_or(RetargetMaterializeError::SourceMissing)?;
            staged_paths.push(staging.copy_to_staging(source, &action.staged_relative_path)?);
        }

        Ok(MaterializedRetarget {
            staged_paths,
            plan: request.plan,
        })
    }
}
```

- [ ] **Step 2: Add materialize test with fake staging**

Append test:

```rust
#[cfg(test)]
mod materialize_retarget_tests {
    use super::*;
    use hmm_core::{
        ReplacementBinding, ReplacementBindingId, ReplacementTargetId, RetargetAction,
    };
    use hmm_ports::StagingFileSystem;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    struct FakeStaging {
        copied: Mutex<Vec<String>>,
    }

    impl StagingFileSystem for FakeStaging {
        fn copy_to_staging(
            &self,
            _source: &Path,
            staged_relative_path: &str,
        ) -> Result<PathBuf, StagingFileSystemError> {
            self.copied
                .lock()
                .expect("lock")
                .push(staged_relative_path.to_owned());
            Ok(PathBuf::from("staging").join(staged_relative_path))
        }
    }

    #[test]
    fn materialize_retarget_uses_staged_relative_path_from_plan() {
        let service = ReplacementService::new(Vec::new());
        let staging = FakeStaging {
            copied: Mutex::new(Vec::new()),
        };
        let plan = RetargetPlan {
            binding: ReplacementBinding {
                id: ReplacementBindingId::new("binding-1"),
                mod_id: "mod-a".to_owned(),
                profile_id: "default".to_owned(),
                source_asset: "pl121_0000".to_owned(),
                target_id: ReplacementTargetId::parse("mhw:armor:fatalis-alpha").expect("id"),
            },
            actions: vec![RetargetAction {
                source_relative_path:
                    "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3".to_owned(),
                staged_relative_path:
                    "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3".to_owned(),
                source_slot: "pl121_0000".to_owned(),
                target_slot: "pl129_0000".to_owned(),
                source_path_family: "pl/f_equip".to_owned(),
                target_path_family: "pl/f_equip".to_owned(),
            }],
            warnings: Vec::new(),
        };

        let materialized = service
            .materialize_retarget(
                &staging,
                MaterializeRetargetRequest {
                    files: vec![PackageFileEntry {
                        relative_path:
                            "nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3".to_owned(),
                        cache_path: PathBuf::from("cache/f_body.mod3"),
                    }],
                    plan,
                },
            )
            .expect("materialized");

        assert_eq!(materialized.staged_paths.len(), 1);
        assert_eq!(
            staging.copied.lock().expect("lock")[0],
            "nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3"
        );
    }
}
```

- [ ] **Step 3: Verify materialize**

Run:

```powershell
cargo test -p hmm-app materialize_retarget
```

Expected:

```text
test result: ok
```

## Task 9: Manifest and InstallPlan Integration（AR3 已实施；实际 snapshot contract 见 AR3 基线）

**Files:**

- Modify: installation MVP manifest model file after it exists
- Modify: installation MVP plan builder file after it exists
- Test: install MVP test crate

- [ ] **Step 1: Add replacement snapshot fields to manifest**

When `InstallManifest` exists, add a replacement snapshot that is immutable after install:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplacementBindingSnapshot {
    pub binding_id: String,
    pub source_slot: String,
    pub target_id: String,
    pub target_slot: String,
    pub source_path_family: String,
    pub target_path_family: String,
    pub target_display_name: String,
    pub retarget_kind: String,
}
```

Add `replacement_bindings: Vec<ReplacementBindingSnapshot>` to `InstallManifest`.

- [ ] **Step 2: Feed staged files into InstallPlan**

The install plan builder must see only final relative paths from staging:

```text
source_ref: staging/nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3
final_relative_path: nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3
```

It must not inspect the original package path to decide conflict keys. Conflict keys come from `final_relative_path`.

- [ ] **Step 3: Add integration tests**

Add tests in the install MVP crate:

```text
retargeted file conflicts by final target path
manifest stores source and target path family
switching target requires old manifest uninstall before new install
```

Expected assertions:

```text
pl121_0000 source package path does not appear as conflict key
pl129_0000 final path appears as conflict key
replacement snapshot contains source_path_family and target_path_family
```

## Task 10: Tauri Commands and DTOs

**Files:**

- Modify: `src-tauri/src/dto.rs`
- Create: `src-tauri/src/replacement_commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/state.rs`
- Test: `cargo test -p hmm-tauri replacement_commands`

- [ ] **Step 1: Add DTOs**

Add DTOs in `dto.rs`:

```rust
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplacementTargetDto {
    pub id: String,
    pub game_id: String,
    pub target_type: String,
    pub display_name_zh_cn: String,
    pub display_name_en: Option<String>,
    pub aliases: Vec<String>,
    pub internal_id: String,
    pub metadata: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplacementBindingDto {
    pub id: String,
    pub mod_id: String,
    pub profile_id: String,
    pub source_asset: String,
    pub target_id: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeReplacementAssetsDto {
    pub game_id: String,
    pub package_id: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRetargetPlanDto {
    pub game_id: String,
    pub package_id: String,
    pub binding: ReplacementBindingDto,
}
```

Mapping rules:

```text
ReplacementTarget.id -> string
ReplacementTarget.internal_id -> display-only string
metadata is passed through, frontend does not branch on MHW path rules
package_id is a backend-owned imported package id
relative package file paths and cache paths are resolved by backend repositories
frontend never sends local cache paths
```

- [ ] **Step 2: Add commands**

Create `replacement_commands.rs`:

```rust
#[tauri::command]
pub fn list_replacement_targets(game_id: String, state: tauri::State<'_, AppState>) -> Result<Vec<ReplacementTargetDto>, CommandErrorDto>;

#[tauri::command]
pub fn analyze_replacement_assets(request: AnalyzeReplacementAssetsDto, state: tauri::State<'_, AppState>) -> Result<ReplacementAnalysisDto, CommandErrorDto>;

#[tauri::command]
pub fn preview_retarget_plan(request: PreviewRetargetPlanDto, state: tauri::State<'_, AppState>) -> Result<RetargetPlanDto, CommandErrorDto>;
```

The command implementation must resolve `package_id` through a backend package repository before calling `ReplacementService`. It must not accept frontend-provided cache paths, parse `plNNN_VVVV`, join `nativePC`, or replace slot strings.

- [ ] **Step 3: Register state and commands**

`AppState` stores:

```rust
pub replacement_service: Mutex<ReplacementService>
```

`lib.rs` registers:

```rust
list_replacement_targets
analyze_replacement_assets
preview_retarget_plan
```

- [ ] **Step 4: Verify command layer**

Run:

```powershell
cargo test -p hmm-tauri replacement_commands
cargo test -p hmm-tauri
```

Expected:

```text
test result: ok
```

## Task 11: Frontend Typed API

**Files:**

- Create: `src/features/replacements/replacementTypes.ts`
- Create: `src/features/replacements/replacementApi.ts`
- Test: `cmd /c corepack pnpm run typecheck`

- [ ] **Step 1: Add frontend types**

Create `replacementTypes.ts`:

```ts
export type ReplacementTarget = {
  id: string;
  gameId: string;
  targetType: string;
  displayNameZhCn: string;
  displayNameEn?: string;
  aliases: string[];
  internalId: string;
  metadata: Record<string, string>;
};

export type ReplacementBindingInput = {
  id: string;
  modId: string;
  profileId: string;
  sourceAsset: string;
  targetId: string;
};

export type AnalyzeReplacementAssetsInput = {
  gameId: string;
  packageId: string;
};

export type PreviewRetargetPlanInput = {
  gameId: string;
  packageId: string;
  binding: ReplacementBindingInput;
};

export type ReplacementAnalysis = {
  sourceSlots: string[];
  sourcePathFamilies: string[];
  supportedRetargetKinds: string[];
  warnings: string[];
};

export type RetargetPlanPreview = {
  actions: Array<{
    sourceRelativePath: string;
    stagedRelativePath: string;
    sourceSlot: string;
    targetSlot: string;
    sourcePathFamily: string;
    targetPathFamily: string;
  }>;
  warnings: string[];
};
```

- [ ] **Step 2: Add typed API**

Create `replacementApi.ts`:

```ts
import { invokeCommand } from "../../shared/api/tauri";
import type {
  AnalyzeReplacementAssetsInput,
  PreviewRetargetPlanInput,
  ReplacementAnalysis,
  ReplacementTarget,
  RetargetPlanPreview,
} from "./replacementTypes";

export function listReplacementTargets(gameId: string) {
  return invokeCommand<ReplacementTarget[]>("list_replacement_targets", { gameId });
}

export function analyzeReplacementAssets(input: AnalyzeReplacementAssetsInput) {
  return invokeCommand<ReplacementAnalysis>("analyze_replacement_assets", input);
}

export function previewRetargetPlan(input: PreviewRetargetPlanInput) {
  return invokeCommand<RetargetPlanPreview>("preview_retarget_plan", input);
}
```

- [ ] **Step 3: Verify frontend typing**

Run:

```powershell
cmd /c corepack pnpm run typecheck
```

Expected:

```text
No TypeScript errors
```

## Task 12: Verification and Safety Review

**Files:**

- Verify: whole workspace

- [ ] **Step 1: Check references**

Run a repository text search for concrete third-party project names, local private tool paths, author names, or wording that implies a runtime dependency on non-project tools.

Expected: no matches. The implementation and docs may describe independently reproducible MHW:I resource rules, but must not cite external project names, local private paths, or third-party ownership details.

- [ ] **Step 2: Check whitespace**

Run:

```powershell
git diff --check
```

Expected: no output.

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

- [ ] **Step 4: Run frontend checks**

Run:

```powershell
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

Expected:

```text
TypeScript, ESLint and Vite build complete without errors
```

- [ ] **Step 5: Run unified verification**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected:

```text
Verification passed.
```

- [ ] **Step 6: Manual safety checklist**

Confirm:

- Tests use temporary directories or fake ports only.
- No test reads or writes a real MHW:I install directory.
- No test reads or writes real player saves.
- No real third-party Mod package is added to the repository.
- No frontend code constructs `nativePC/pl/f_equip` paths.
- No core or app code branches on `plNNN_VVVV`; that validation stays inside `hmm-games-mhw`.
- Retarget materialize writes staging only.
- Install conflicts use final staged target path, not source package path.

## Execution Order

Recommended commit slices:

```text
1. core replacement model
2. replacement ports
3. MHW catalog and Unicode normalization
4. MHW armor path parser
5. MHW RetargetPlan builder
6. application replacement service
7. staging materialize
8. manifest and InstallPlan integration
9. Tauri commands
10. frontend typed API
11. verification fixes
```

Do not combine manifest integration with frontend UI work in the same commit. Do not start UI screens until command DTOs and backend preview tests are green.

## Risk Register

| Risk | Mitigation |
|------|------------|
| Catalog 数据不完整 | MVP seed 只覆盖测试与演示；完整 catalog 作为独立数据任务补齐。 |
| 中文名搜索误命中 | UI 搜索使用 normalized alias 辅助展示，唯一绑定始终使用 `ReplacementTarget.id`。 |
| 文件名包含 slot 裸数字被误改 | 路径 parser 结构化分段，单测覆盖 `f_121_0000_extra.mod3`。 |
| `m_equip` 被静默跳过 | analyzer 返回 warning，plan builder 阻止自动 retarget。 |
| staging 路径逃逸 | `StagingFileSystem` 拒绝绝对路径、prefix、root、parent segment。 |
| core 污染 MHW 规则 | `plNNN_VVVV` 校验和 `nativePC` 路径解析只在 `hmm-games-mhw`。 |
| 安装冲突看错路径 | manifest 和 InstallPlan 使用 staged final relative path。 |

## Completion Criteria

实现完成时必须满足：

- `list_replacement_targets("mhw")` 返回 MHW armor catalog seed。
- `analyze_replacement_assets` 能识别 `f_equip` 源 slot，并对 `m_equip` 给出 warning。
- `preview_retarget_plan` 对 `pl121_0000 -> pl129_0000` 生成结构化 action。
- 文件名包含 `121_0000` 时，retarget 后文件名保持不变。
- `materialize_retarget` 只写 staging，不触碰游戏目录。
- manifest snapshot 包含 `source_path_family` 和 `target_path_family`。
- 统一验证脚本通过。
