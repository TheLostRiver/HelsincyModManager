# Steam 游戏目录自动扫描候选列表设计

## 背景

当前项目已经完成《怪物猎人：世界 冰原》首次启动游戏目录配置闭环：玩家可以手动选择目录，后端通过 MHW:I adapter 校验目录，再把配置保存到应用数据目录。`GameDiscoveryService` 和前端“自动扫描 Steam”按钮已经存在，但真实实现仍是 `NoopGameDiscoveryService`，应用层也会丢弃扫描结果。

本设计覆盖下一条功能线：从 Steam 安装信息中扫描 MHW:I 的候选游戏目录，并把候选列表返回给前端展示。玩家可以从候选列表中选择一个目录保存；保存动作仍复用现有 `save_game_directory` 流程，避免扫描结果绕过目录校验。

## 目标

- 自动读取 Steam library 信息，扫描 MHW:I 候选安装目录。
- 将候选目录以列表形式返回前端，而不是自动保存。
- 每个候选目录都经过 MHW:I adapter 校验，并携带校验结果、可信度和来源信息。
- 前端展示候选列表，玩家点击有效候选后保存目录。
- 保留手动选择目录作为兜底路径。
- Windows 优先可用，同时通过平台抽象为 Linux / Steam Deck 保留扩展空间。
- Steam app id、游戏显示名等游戏差异由 adapter 或 catalog 提供，不写死在通用 discovery 逻辑中。

## 非目标

- 不实现进程扫描。
- 不实现一键启动游戏。
- 不自动写入游戏配置。
- 不写入游戏安装目录。
- 不读取或修改玩家存档。
- 不扫描整块磁盘寻找可执行文件。
- 不实现 Mod 导入、安装、卸载、备份或回滚。
- 不为 Steam Deck 做实机验证；本轮只保留路径和接口边界。

## 方案比较

### 方案 A：前端解析 Steam 路径

前端调用 Tauri 文件 API 或命令读取 Steam 配置，然后在 React 中拼出游戏目录。

优点是 UI 实现直观，缺点是直接违反项目边界：前端会承担平台路径、Steam VDF、游戏 app id 和目录规则。后续扩展 Rise / Wilds 或 Steam Deck 时，前端会变成规则堆积点。

结论：不采用。

### 方案 B：基础设施发现候选，应用层校验候选

`hmm-infra` 负责 Steam root、libraryfolders.vdf、appmanifest_582010.acf 等平台和文件读取；`hmm-app` 负责拿到 raw candidate 后调用对应 `GameAdapter` 校验；Tauri command 只做 DTO 映射；前端只展示候选并触发保存。

优点是边界清晰：Steam 细节在 infra，游戏目录规则在 adapter，前端只消费结构化结果。保存时再次走现有校验流程，能够防止扫描结果过期或被篡改。

结论：推荐采用。

### 方案 C：把 Steam 扫描做成游戏 adapter 能力

每个游戏 adapter 自己扫描 Steam library 并返回候选目录。

优点是每个游戏可以完全自定义扫描规则。缺点是 Steam 解析、平台 root 识别、VDF 解析会在 MHW:I、Rise、Wilds adapter 中重复出现，后续维护成本高。

结论：不作为首选。只有某个游戏存在非常特殊的 launcher 识别规则时，再通过扩展接口让 adapter 参与。

## 推荐设计

采用方案 B。

总体流程：

```text
前端点击“自动扫描 Steam”
  -> scan_game_candidates("mhw")
  -> Tauri command 解析 game id
  -> hmm-app 获取 MHW:I adapter
  -> adapter 提供 Steam app id 和显示名
  -> hmm-infra 扫描 Steam library 并返回 raw candidates
  -> hmm-app 使用 MHW:I adapter 校验每个候选目录
  -> Tauri DTO 返回候选列表
  -> 前端展示列表
  -> 玩家选择有效候选
  -> save_game_directory("mhw", candidate.directory)
  -> 后端重新校验并保存
```

这个流程的关键点是：扫描只能发现候选，不能替代保存前校验。

## 后端分层

### Ports

`hmm-ports::game_setup` 继续承载游戏目录配置相关接口。本轮建议把 discovery 输入和输出显式建模：

```rust
pub struct GameDiscoveryRequest {
    pub game_id: GameId,
    pub display_name: String,
    pub steam_app_id: Option<u32>,
}

pub struct GameCandidate {
    pub game_id: GameId,
    pub display_name: String,
    pub root_dir: PathBuf,
    pub source: GameCandidateSource,
    pub source_label: String,
}

pub enum GameCandidateSource {
    Steam,
}
```

`GameDiscoveryService` 接收 `GameDiscoveryRequest`，避免基础设施层根据 `GameId` 写死 MHW:I app id。

`GameAdapter` 建议增加默认方法：

```rust
fn steam_app_id(&self) -> Option<u32> {
    None
}
```

MHW:I adapter 返回 `Some(582010)`。后续 Rise / Wilds 只需要在各自 adapter 中声明自己的 app id。

### Infra

`hmm-infra` 新增 Steam discovery 模块，职责分为四块：

- `SteamRootProvider`：识别 Steam 安装根目录。
- `SteamLibraryParser`：解析 `steamapps/libraryfolders.vdf`。
- `SteamAppManifestParser`：解析 `steamapps/appmanifest_<app_id>.acf`。
- `SteamGameDiscoveryService`：把 root、library、manifest 串起来，生成 raw candidates。

Windows root 识别优先级：

1. Windows 注册表中的 Steam 安装路径。
2. 常见默认目录，例如 `C:\Program Files (x86)\Steam`。
3. 如果未找到 Steam root，返回空候选列表，不把它当成存储错误。

Linux / Steam Deck 预留路径：

- `~/.steam/steam`
- `~/.local/share/Steam`
- Flatpak Steam 目录：`~/.var/app/com.valvesoftware.Steam/.local/share/Steam`

本轮可以实现可测试的路径生成逻辑，但不声称已经完成 Steam Deck 实机验证。

### App

`GameSetupService::scan_candidates` 从返回 `Result<(), ...>` 改为返回候选扫描结果。应用层负责：

- 根据 game id 找到 adapter。
- 从 adapter 读取 Steam app id。
- 调用 discovery service 扫描 raw candidates。
- 使用 `GameDirectoryProbeFactory` 和 adapter 校验每个候选。
- 去重、排序，并把有效候选排在前面。

建议排序规则：

1. 有效候选优先。
2. 校验可信度高的优先。
3. Steam 来源稳定排序，避免 UI 列表抖动。

### Tauri

`scan_game_candidates` 返回结构化 DTO：

```rust
pub struct GameCandidateScanDto {
    pub game_id: String,
    pub candidates: Vec<GameCandidateDto>,
}

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

`directory` 用于玩家点击候选后保存；日志和诊断不得记录完整路径。`path_label` 用于常规展示。

### Frontend

前端新增候选列表状态：

```ts
type GameDirectoryCandidate = {
  gameId: GameId;
  displayName: string;
  directory: string;
  pathLabel: string;
  source: "steam";
  sourceLabel: string;
  isValid: boolean;
  confidence: number;
  errors: GameSetupErrorCode[];
};
```

`useGameSetup` 的 `scanSteam` 改为接收候选结果并写入 state。`GameDirectoryActions` 继续负责按钮；新增 `GameDirectoryCandidateList` 展示候选并把有效候选传给 `saveDirectory`。

UI 行为：

- 扫描中禁用扫描按钮和候选选择按钮。
- 扫描成功且有候选：展示候选列表。
- 扫描成功但无候选：提示未发现 Steam 候选，保留手动选择按钮。
- 候选有效：允许点击“使用此目录”。
- 候选无效：展示错误原因，按钮禁用。
- 点击候选后仍调用 `save_game_directory`，由后端重新校验后保存。

## VDF / ACF 解析策略

Steam 的 `libraryfolders.vdf` 和 `appmanifest_<app_id>.acf` 是结构化 KeyValues 文件。本项目不应通过零散字符串 contains 判断完成解析。

首版建议在 `hmm-infra` 中实现一个小型、受限的 KeyValues parser：

- 支持双引号 key/value。
- 支持嵌套对象。
- 支持忽略空白。
- 对损坏输入返回解析错误。
- 单元测试覆盖 Windows 路径、转义字符、缺少 manifest、缺少 `installdir`。

该 parser 只服务 Steam discovery，不扩散到业务层。如果后续引入成熟 parser crate，应保持模块 API 不变。

## 安全与隐私

- 不扫描整个磁盘。
- 不写入 Steam 目录、游戏目录或存档目录。
- 不把完整本地路径写入日志。
- 不读取真实 Mod 包。
- 不自动保存扫描结果。
- 保存候选前必须重新校验目录。
- 测试使用临时目录和人工构造的 Steam 文件，不依赖真实 Steam 安装。

## 并发

首版 Steam 扫描可以同步完成。扫描范围只限 Steam root 和 library manifest，通常是少量文件读取。

后续如果候选数量增多或加入进程扫描，可以迁移到后台任务系统。迁移时仍需遵守：

- 多个 library 读取可以并行。
- 同一 game id 的保存操作串行。
- 进度事件必须携带 task id。
- 不在扫描阶段持有游戏写锁。

## 错误处理

推荐语义：

- Steam root 未找到：返回空候选，不报错。
- `libraryfolders.vdf` 缺失：返回空候选，不报错。
- 某个 library manifest 损坏：跳过该 library，并在后端测试中覆盖；首版 UI 可不展示细粒度 notice。
- 指定 app manifest 缺失：说明该 library 未安装目标游戏，跳过。
- app manifest 存在但候选目录无效：返回候选，但标记 `is_valid = false`。
- discovery 发生不可恢复 I/O 错误：返回 `scan_failed`，前端展示可读错误。

## 测试策略

Rust：

- `hmm-infra`：KeyValues parser 单元测试。
- `hmm-infra`：临时目录构造 Steam root、libraryfolders.vdf、appmanifest_582010.acf，验证候选路径。
- `hmm-infra`：缺失 Steam root、缺失 manifest、损坏 VDF 的行为。
- `hmm-app`：fake discovery + fake adapter，验证候选会被 adapter 校验、排序和返回。
- `hmm-games-mhw`：验证 `steam_app_id()` 返回 `582010`。
- Tauri DTO：验证候选 DTO 不丢失校验结果。

前端：

- `scanGameCandidates` 返回候选 DTO 类型正确。
- `useGameSetup.scanSteam` 能更新候选列表。
- 无候选时展示手动选择兜底文案。
- 无效候选不能触发保存。
- 有效候选点击后复用 `saveDirectory`。

统一验证：

```powershell
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
cargo test --workspace
cargo check --workspace
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

## 验收标准

- 点击“自动扫描 Steam”后，前端能显示候选列表或明确无候选提示。
- MHW:I Steam app id 只由 MHW:I adapter 或游戏 catalog 提供。
- Steam VDF / ACF 解析只在 infra 模块内。
- 前端不包含 `appmanifest_582010.acf`、`libraryfolders.vdf`、`MonsterHunterWorld.exe` 等发现规则。
- 扫描结果不会自动保存。
- 保存候选时仍走现有 `save_game_directory` 校验。
- 测试不依赖真实 Steam、真实游戏目录或真实存档。
