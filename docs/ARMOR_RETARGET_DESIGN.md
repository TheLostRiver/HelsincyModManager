# MHW:I 外观套装重定向设计

> 本文档已吸收 [`ARMOR_RETARGET_REVIEW.md`](ARMOR_RETARGET_REVIEW.md) 的 P0/P1/P2 评审意见（catalog 主键分层、Unicode 归一化、结构化分段替换、m/f_equip 区分、变体建模、核心层边界等）。
>
> 实施状态（2026-07-16）：阶段 1 / AR1、阶段 2 / AR2、阶段 3 / AR3 与阶段 4 / AR4 已标记为 `implemented`。
> 当前已落地稳定 replacement identity/binding、只读 catalog port、`mhw-armor-v1` catalog、严格
> parser、单 source analyzer、纯 `RetargetPlan`、受控 staging materialize、InstallPlan 与 binding
> snapshot 集成、四个窄 Tauri command 和 Mod 详情受控 UI；当前下一项为阶段 5 / AR5。真正重装
> target switch、卸载闭环与 Gate B 验收尚未完成。

## 背景

很多《怪物猎人：世界 冰原》外观 Mod 并不是新增一套独立外观，而是把自定义模型、贴图或材质覆盖到官方装备槽位。玩家真正关心的工作流是：

```text
导入一个外观 Mod
选择它要替换哪套官方装备或幻化
安装后进入游戏装备该官方套装时看到 Mod 效果
```

例如一个 Mod 作者把“红色礼服裙”打包为替换守护者套装，玩家希望它改为替换黑龙套装。管理器需要把这个“Mod 资源 -> 官方套装槽位”的绑定关系建模为一等概念，而不是把它当成一次性的文件改名工具。

前期调研验证了 MHW:I 常见外观包的基础机制：把包内 `nativePC/pl/f_equip/<plNNN_VVVV>/...` 路径中的套装编号替换为目标套装编号，再输出可安装内容。类似一次性路径改写做法可以帮助理解资源槽位规则，但缺少压缩包安全、manifest、备份、回滚、日志脱敏和跨平台路径规范化，不能作为运行时依赖直接接入本项目。

## 设计目标

- 支持玩家为 MHW:I 外观 Mod 选择官方 armor replacement target。
- 第一版覆盖路径级重定向：`nativePC/pl/f_equip/<source_slot>/...` 到 `nativePC/pl/f_equip/<target_slot>/...`。
- 原始导入包保持只读；所有重定向结果只在 staging 目录生成。
- 安装仍然必须经过 `InstallPlan`、冲突检测、备份、manifest 和 rollback。
- MHW:I 资源编号、路径族和套装 catalog 收敛在 `hmm-games-mhw` 或其数据资源中。
- 前端只消费后端提供的 catalog、分析结果和计划预览，不拼接路径、不改写编号。
- 为未来武器、语音、更复杂二进制 transformer 预留扩展点。

## 非目标

第一版不解决以下问题：

- 直接运行或包装外部 Python 工具。
- 修改 `.mod3`、`.mrl3`、`.tex` 等文件内部可能存在的二进制引用。
- 自动拆分多个源套装混合在同一 Mod 包里的复杂场景。
- 自动判断男女体差异、所有部位缺失策略或作者自定义目录语义。
- 武器、语音、NPC、随从外观的重定向。
- 把改写后的 zip 作为唯一安装事实来源。

这些能力可以在路径级 armor retarget 稳定后，通过更细的 transformer 能力迭代。

## 术语

`ReplacementTarget`：官方游戏资源槽位。对 MHW:I armor 来说，它通常对应一个 `plNNN_VVVV` 套装编号。

`ReplacementBinding`：玩家选择的绑定关系，描述某个 Mod 的源资源槽位应安装到哪个官方目标槽位。

`RetargetPlan`：在 staging 目录中生成安装变体所需执行的改写计划。它不直接修改游戏目录。

`RetargetAction`：单个 staging 改写动作，例如把一个源相对路径复制到改写后的目标相对路径。

`staging`：安装前的临时生成目录。重定向、路径改写和安装变体都发生在这里。

> 路径结构事实：MHW:I armor 资源在 `<slot>` 之后是**固定结构目录** `arm/mod`，即 `nativePC/pl/f_equip/<slot>/arm/mod/<filename>`。部位（头/胸/手/腰/脚）不作为独立目录段出现。因此 `part` 在本设计中只是 catalog 的逻辑标签，由游戏 adapter 在 `metadata` 中维护，不参与路径改写。retarget 永远只改写 `<slot>` 这一段。

## 推荐路线

采用“正式建模 + 路径级 MVP + 可扩展 transformer”的路线：

1. 先在领域层和端口层补齐 replacement/retarget 概念。
2. 在 `hmm-games-mhw` 中提供 MHW:I armor catalog 和路径级 analyzer/transformer。
3. 在应用层把 retarget 插入导入与安装计划生成之间。
4. 在前端启用替换目标选择与冲突预览。
5. 等路径级 MVP 稳定后，再扩展高级 transformer。

不建议直接做完整 transformer 框架，也不建议包装外部 Python 工具。前者会过早扩大范围，后者会绕过本项目安全边界。

## 模块边界

### `hmm-core`

负责通用领域模型，不知道 MHW:I 的路径和编号。

建议模型：

```text
ReplacementTarget
  id              // 项目稳定主键，游戏无关，全局唯一（如 "mhw:armor:fatalis-alpha"）
  game_id         // 所属游戏
  target_type     // armor / weapon / voice / ...
  display_name    // 展示用名称（可按语言分字段）
  aliases         // 别名数组，仅用于展示和检索，不参与匹配
  internal_id     // 游戏 adapter 的槽位编号；对 MHW armor 形如 plNNN_VVVV
                  // 仅在 game_id + path_family 范围内唯一
  metadata        // 游戏专属字段（path_family / rank / variant / part /
                  // is_full_body / monster / parts 等），核心层透传不解析

ReplacementBinding
  id
  mod_id
  profile_id
  source_id       // 稳定、游戏无关且对 core 不透明；AR2 负责从单 source 分析结果生成
  target_id       // 引用 ReplacementTarget.id（项目主键），不引用 internal_id
  created_at_unix_millis

ReplacementCatalog
  version         // catalog 数据版本，不等同于游戏版本或 internal_id
  game_id
  targets

RetargetPlan
  binding
  actions
  warnings

RetargetAction
  action_type
  source_relative_path
  staged_relative_path
  source_slot            // 源槽位编号（如 pl121_0000）
  target_slot            // 目标槽位编号（如 pl129_0000）
  source_path_family     // 如 pl/f_equip 或 pl/m_equip
  target_path_family     // 如 pl/f_equip 或 pl/m_equip
```

字段边界约束：

- `id` 是项目自身的稳定主键，游戏无关。`internal_id` 是游戏 adapter 的槽位编号（MHW armor 形如 `plNNN_VVVV`），仅在 `game_id + path_family` 范围内唯一，绝不当全局主键。理由：武器替换、语音替换、Rise/Wilds 的编号形态都不是 `plNNN_VVVV`，把 MHW 形态绑死成全局主键会污染多游戏场景。`ReplacementBinding.target_id` 引用的是 `ReplacementTarget.id`，不是 `internal_id`。
- 所有游戏专属语义字段（`path_family`、`rank`、`variant`、`part`、`is_full_body`、`monster`、`parts`）一律放进 `metadata`，由对应游戏 adapter 解析。核心层不对 `metadata` 内任何字段值做分支判断，也不校验 `plNNN_VVVV` 这种游戏专属格式。
- `RetargetAction` 携带 `source_path_family` / `target_path_family`，用于区分男女体路径（`f_equip` / `m_equip`），manifest 和冲突检测据此区分同名 slot。

### `hmm-ports`

负责声明应用层依赖的 trait。

AR1 已落地的只读能力：

```text
ReplacementCatalogProvider
  replacement_catalog() -> ReplacementCatalog
  find_replacement_target(target_id) -> ReplacementTarget
  search_replacement_targets(query) -> Vec<ReplacementTarget>
```

AR2 已扩展纯 analysis/plan port；AR3 已增加独立的 batch staging port：

```text
ReplacementAdapter（AR2）
  analyze_replacement_assets(package) -> ReplacementAnalysis
  build_retarget_plan(request) -> RetargetPlan

RetargetStagingMaterializer（AR3）
  materialize(files: RetargetStagingFile[])

RetargetStagingFile（AR3）
  package_file_id + final InstallTargetPath
```

只读 catalog port 与目录校验 `GameAdapter` 保持分离，避免迫使不支持 replacement 的 adapter 实现
空方法。AR2 已在该基础上新增更窄的 analysis/plan port，没有把 path 或 filesystem 类型反向塞回
AR1；AR3 的 staging I/O 继续独立建模，core/app 不携带 staging root 或 `PathBuf`。

### `hmm-games-mhw`

负责 MHW:I 专属规则：

- `data/mhw-armor-targets.v1.json` armor catalog 数据、schema/catalog version 与加载校验。
- catalog 加载时的 Unicode 归一化：对 display name 至少做 `NFC` 归一化，并对"看起来都像中点"的码位 `U+2027`（间隔号）/ `U+00B7`（中点）/ `U+30FB`（全角中点）/ `U+FF65`（半角中点）建立显式归一化映射表。归一化规则只存在于 adapter 内，核心层不感知。
- `pl/f_equip/<slot>` 和 `pl/m_equip/<slot>` 路径族识别（两者为不同 path_family）。
- `plNNN_VVVV` 编号解析与校验。该格式校验**只在 adapter 内做**，核心层把 `internal_id` 当不透明字符串。
- catalog schema 内的游戏专属字段（`path_family` / `rank` / `variant` / `part` / `is_full_body` / `monster` / `parts`）由 adapter 解析并写入 `ReplacementTarget.metadata`，核心层透传。
- 源槽位推断。
- 路径级 `RetargetPlan` 生成。
- 黑龙/煌黑龙等名称和别名区分（见 [Catalog 设计](#catalog-设计)）。

通用核心和前端都不应写死 `pl129_0000`、`nativePC`、`f_equip` 等规则。

### `hmm-app`

负责编排用例：

```text
导入 Mod
安全解压和包分析
保存原始包引用和元数据
读取当前游戏 replacement catalog
接收玩家 ReplacementBinding
调用游戏 adapter 生成 RetargetPlan
在 staging 生成安装变体
基于 staging 生成 InstallPlan
执行安装、备份、manifest、rollback
```

应用层不直接操作真实文件系统，不直接解析 MHW:I 编号。

### `hmm-infra`

负责真实 I/O 和安全约束：

- sandbox 解压。
- staging 目录创建和清理。
- 路径规范化。
- 防止路径穿越、绝对路径、大小写冲突。
- hash 计算。
- 文件复制和原子写入辅助。

基础设施不理解“黑龙套装”或 `pl129_0000` 的业务含义。

### 前端

前端的 `features/replacements/` 只负责展示和交互：

- 展示 Mod 分析出的源槽位。
- 展示当前游戏的官方目标 catalog。
- 支持搜索、筛选、别名和内部编号展示。
- 展示安装前最终路径冲突预览。
- 把玩家选择提交给后端生成 binding 或 plan。

前端不拼接路径，不替换字符串，不根据游戏名分支安装规则。

## Catalog 设计

MHW:I armor catalog 应使用 JSON 或 TOML 存储，并由 `hmm-games-mhw` 加载。建议先使用静态随包数据，后续再考虑社区补丁或版本化更新。

顶层字段保持游戏无关（与 `hmm-core` 的 `ReplacementTarget` 一致），游戏专属字段放进 `metadata`：

```json
{
  "id": "mhw:armor:fatalis-alpha",
  "game_id": "mhw",
  "target_type": "armor",
  "display_name": {
    "zh_cn": "【精英‧龙α】服装",
    "en": "Fatalis Alpha +"
  },
  "aliases": ["黑龙α", "黑龙 Alpha", "Fatalis α"],
  "internal_id": "pl129_0000",
  "metadata": {
    "path_family": "pl/f_equip",
    "monster": "fatalis",
    "rank": "master",
    "variant": "alpha",
    "is_full_body": false,
    "parts": ["head", "body", "arms", "waist", "legs"]
  }
}
```

主键与匹配规则：

- `id`（如 `mhw:armor:fatalis-alpha`）是项目稳定主键，全局唯一、游戏无关。`internal_id`（如 `pl129_0000`）是 MHW adapter 的槽位编号，仅在 `game_id + path_family` 范围内唯一，用作 retarget 匹配键，绝不当全局主键。`ReplacementBinding.target_id` 引用的是 `id`。
- 中文名和别名仅用于展示和检索，不参与 join 或匹配。

`metadata` 内的变体建模：

- `rank` 取值枚举：`high`（上位）、`master`（精英/冰原）、`event`（活动/换色）、`gamma`（γ 套装）。同一个怪物的上位、精英、活动、γ 套装分别对应不同的 `internal_id`。
- `variant` 取值枚举：`alpha` / `beta` / `gamma`，只表示 α/β/γ 三种变体。
- 亚种（如火龙魂、暴君角龙、雷颚龙）和活动换色（如银白耀日、死灭与繁荣）不塞进 `rank` 或 `variant`，而作为**独立的 `ReplacementTarget` 条目**——它们本就拥有独立的 `plNNN_VVVV`，应有独立的 `id`、`display_name` 和 `monster` 字段。
- UI 筛选维度应是 `rank × variant × monster`，而不是 `variant` 单维度。

黑龙 / 煌黑龙的 Unicode 陷阱（必须显式处理）：

```text
【精英‧龙α】服装     -> Fatalis   / 黑龙α     -> pl129_0000   分隔符 ‧ = U+2027
【精英‧龙β】服装     -> Fatalis   / 黑龙β     -> pl129_0010   分隔符 ‧ = U+2027
【精英·煌黑龙α】服装 -> Alatreon  / 煌黑龙α  -> pl052_0000   分隔符 · = U+00B7
【精英·煌黑龙β】服装 -> Alatreon  / 煌黑龙β  -> pl052_0010   分隔符 · = U+00B7
```

注意 Fatalis 与 Alatreon 的分隔符是**不同码位**：前者 `‧`(U+2027 间隔号)，后者 `·`(U+00B7 中点)，视觉上几乎相同但码位完全不同。此外怪物名也不同（`龙` vs `煌黑龙`）。因此：

- catalog 加载必须做 Unicode 归一化（见 [hmm-games-mhw](#hmm-games-mhw) 职责），否则玩家或运营手抄中文名时把 `·`(U+00B7) 误打成 `‧`(U+2027) 会导致 `internal_id` 查不到。
- UI 搜索匹配必须基于 `metadata.monster` 逻辑字段（`fatalis` / `alatreon`），**不基于中文名子串**。用怪物名 `龙` 做子串匹配会同时命中 Fatalis 和所有含`龙`字条目。
- alias 数组与 display name 之间的归一化（全半角、希腊字母 α vs `Alpha`、中点码位）规则由 adapter 统一处理，核心层不感知。

## 包分析

包分析器应在安全解压后的 sandbox/cache 中分析相对路径。第一版只识别规范化后的路径：

```text
nativePC/pl/f_equip/<slot>/arm/mod/<filename>
```

其中 `<slot>` 必须匹配：

```text
pl[0-9]{3}_[0-9]{4}
```

路径规范化与 path_family 识别：

- 路径分隔符 `/` 和 `\` 必须先统一规范化再匹配，否则从 zip 读出的正斜杠路径和从 rar 读出的反斜杠路径会产生两种分析结果。
- `pl/f_equip/` 和 `pl/m_equip/` 视为**两个不同的 path_family**。第一版只支持 `f_equip` retarget；含 `m_equip` 文件的包按多源处理（见下），给出警告并阻止自动 retarget，不悄悄只处理 `f_equip`。
- `<slot>` 之后的 `arm/mod` 是**固定结构目录**，不是部位。分析器不试图从路径目录推断部位。

分析输出建议：

```text
ReplacementAnalysis
  detected_targets
  source_slots
  source_path_families   // 区分 f_equip / m_equip
  supported_retarget_kinds
  warnings
```

对于多个源 slot 或多个 path_family：

- 若全部属于同一 slot 且同一 path_family（`f_equip`），允许生成 retarget。
- 若发现多个 armor slot、或同时存在 `f_equip` 与 `m_equip`、或含 `m_equip`，第一版给出警告并阻止自动 retarget。
- 后续可以支持多 binding 或高级拆分。

## RetargetPlan 生成

路径级 retarget 的规则：

```text
source: nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3
target: nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3
```

**实现规则（强制）：结构化分段替换，不是字符串替换。**

MHW:I armor 路径的段结构是固定的：

```text
nativePC / pl / f_equip / <slot=plNNN_VVVV> / arm / mod / <filename>
```

生成 plan 时：

- 把路径解析成上述段。
- **只允许替换 `<slot>` 段**。`nativePC`、`pl`、`f_equip`、`arm`、`mod`、`<filename>` 一律原样保留。
- 重新 join 成目标路径。禁止用整路径字符串替换、禁止 `replace(slot_a, slot_b)` 这种宽泛做法。

之所以写成强制规则，是因为宽泛字符串替换会误伤碰巧包含同样数字的片段。例如源 slot `pl121_0000` 的裸数字是 `121_0000`，若 filename 恰好是 `f_121_0000_extra.mod3`，整路径 `replace("121_0000", "129_0000")` 会把 filename 也改成 `f_129_0000_extra.mod3`，这是错误的。结构化分段替换能避免这类误伤。

生成 plan 时还需要：

- 校验 source slot 来自包分析结果。
- 校验 target `id` 存在于当前游戏 catalog。
- 校验 source 和 target path 都是相对路径。
- 校验 source 与 target 的 path_family 一致（`f_equip` 不重定向到 `m_equip`）。
- 保留原始文件内容，只改变 staging 目标路径。
- 记录 warning，例如目标和源相同、多个源 slot、缺失常见部位。

## Staging 与安装计划

retarget 后的 staging 应成为 `InstallPlan` 的输入，而不是直接输出 zip。

推荐流程：

```text
ArchiveInspect
SandboxExtract
PackageAnalyze
ReplacementBinding
RetargetPlan
StagingMaterialize
InstallPlan
ConflictCheck
Commit
ManifestWrite
```

`StagingMaterialize` 只在应用数据目录或临时目录下生成文件，不触碰游戏目录。`InstallPlan` 看到的是最终将写入游戏目录的相对路径，因此冲突检测天然基于最终目标槽位。

如果玩家切换目标：

```text
卸载旧 binding 的 manifest
重新生成新 binding 的 staging
安装新 InstallPlan
```

不要在游戏目录中尝试“原地改名”。

staging 目录的生命周期：它是**临时生成物**，可丢弃、可重建。切换目标或回滚后，旧 staging 可以安全清理。安装事实的唯一来源是三者：**原始导入包（只读）+ `ReplacementBinding` + `InstallManifest`**。任何时刻只要这三者在，就能重新 materialize 出 staging。这与 `ARCHITECTURE.md` “原始导入包永远只读”一致。staging 本身不作为事实来源持久化。

## Manifest 与审计

安装 manifest 必须记录 replacement 信息：

```text
replacement_bindings
  binding_id
  source_slot
  target_id
  target_slot
  source_path_family     // 如 pl/f_equip，区分男女体
  target_path_family
  target_display_name
  retarget_kind

installed_files
  final_relative_path
  source_package_path
  staged_relative_path
  hash
  size

backups
  final_relative_path
  backup_ref
  previous_hash
```

Audit Log 应记录：

- 生成 retarget plan。
- materialize staging。
- commit 安装。
- 覆盖、备份、删除、rollback。

日志只记录脱敏路径、内部 ID、hash、大小和错误分类，不记录完整本地路径或第三方 Mod 内容。

## 冲突规则

冲突检测基于最终目标路径：

```text
nativePC/pl/f_equip/pl129_0000/...
```

不是基于原包路径：

```text
nativePC/pl/f_equip/pl121_0000/...
```

典型冲突：

- 两个 Mod retarget 到同一 target slot 且写入同一文件。
- 同一个 profile 中已有 Mod 占用目标路径。
- 目标路径上存在非 manifest 管理文件，需要提示覆盖风险。
- 同一个 Mod 切换 target 时旧 binding 尚未卸载。

第一版可把同一路径写入视为 hard conflict，除非后续设计明确支持覆盖优先级或 load order。

## 错误处理

建议错误分类：

```text
UnsupportedReplacementTarget
UnrecognizedSourceSlot
AmbiguousSourceSlot
UnsafeRetargetPath
TargetCatalogMissing
RetargetWouldOverwriteManagedFile
RetargetWouldOverwriteUnmanagedFile
StagingMaterializeFailed
```

所有错误都应可展示给前端，并能说明玩家下一步可以做什么。例如：

- “这个 Mod 包里检测到多个外观槽位，当前版本不能自动重定向。”
- “目标套装 catalog 中不存在所选槽位，请更新 catalog 或选择其他目标。”
- “目标路径已经被另一个 Mod 占用，请先禁用冲突 Mod。”

## UI 工作流

首版主要入口固定为 `Mod 管理 -> Mod 详情统一面板 -> 替换目标 Tab`；现有右键“MOD 文件修改”动作
直接打开该 Tab。该入口承载具体 Mod 的 source、target 选择和预览；未来 `/replacements` 页面只做
全局 binding、占用与冲突总览，不作为 Gate B 首个操作入口。

建议流程：

1. 玩家导入外观 Mod。
2. 包分析结果显示“检测到源槽位：守护者 `pl121_0000`”。
3. 玩家进入替换目标选择。
4. UI 展示 armor catalog，可按大师位、α/β、活动、联动、怪物名筛选。
5. 玩家选择“黑龙 α / Fatalis Alpha+ / `pl129_0000`”。
6. 后端生成预览计划。
7. UI 展示最终写入路径摘要和冲突状态。
8. 玩家确认安装。

UI 中“黑龙”和“煌黑龙”必须清晰区分，不能只显示模糊别名。

## 数据迁移与持久化

SQLite 中应持久化玩家状态：

- replacement binding。
- profile 与 binding 的关系。
- 安装 manifest 中的 replacement 记录。

静态 catalog 不建议存在 SQLite 初始版本里。它属于游戏规则数据，应随 `hmm-games-mhw` 版本化发布。若未来支持用户自定义 catalog，再设计导入/覆盖层。

## 测试策略

单元测试：

- catalog 中每个 `internal_id` 符合 `plNNN_VVVV`。（**该测试落在 `hmm-games-mhw`，不落在 `hmm-core`**——核心层把 `internal_id` 当不透明字符串，不校验游戏专属格式。）
- 中点码位归一化：把 `‧`(U+2027) 和 `·`(U+00B7) 两种写法的 display name 作为输入，验证 catalog 能查到同一条记录（`internal_id` 相同）。
- `黑龙` 和 `煌黑龙` alias 不混淆；搜索“黑龙”只命中 `monster == fatalis` 的条目，不串到 `alatreon`。
- path parser 能识别 `/` 和 `\` 输入并规范化。
- 多源 slot、或同时含 `f_equip` 与 `m_equip`、或含 `m_equip`，返回 `AmbiguousSourceSlot`。
- 未知 target id 返回 `TargetCatalogMissing`。
- retarget action 只替换 slot 段，不改其他路径片段。**特别地：当 filename 或父目录里恰好包含与 source slot 相同的数字串时，只有真正的 slot 段被替换**（构造样本：`.../pl121_0000/arm/mod/f_121_0000_extra.mod3`，验证 filename 不变）。
- 同一 `plNNN` 前缀下若存在多个 `VVVV`（如 `pl033_0000/0010/0100/...`），必须每条都有可区分的 `display_name` + 独立 `id`，不共用一个 `variant`。

集成测试：

- 使用人工构造 zip，不提交真实第三方 Mod。
- 在临时目录模拟 sandbox、staging 和游戏目录。
- 覆盖 `pl121_0000 -> pl129_0000` 的完整计划生成。
- 两个 Mod 写入同一最终路径时冲突检测生效。
- 切换 target 时旧 manifest 卸载、新 binding 安装。
- 安装失败时 rollback 恢复临时游戏目录状态。

安全测试：

- 路径穿越样本被拒绝。
- 绝对路径样本被拒绝。
- 大小写冲突样本被拒绝。
- staging 不会写出应用控制目录。
- 日志不包含完整本地路径或第三方 Mod 内容。

## 分阶段落地

### 阶段 1：模型与 catalog（AR1，已实现）

- 已定义 stable replacement target/binding/source/catalog identity；analysis/plan 模型留到 AR2。
- 已为 MHW:I 建立 `mhw-armor-v1` 最小 catalog seed。
- 已提供 catalog list/find/search、serde 不变量、schema、Unicode 和精确搜索校验测试。

### 阶段 2：包分析与路径级 RetargetPlan（AR2，已实现）

- 分析 `nativePC/pl/f_equip/<slot>`（含 `/` 和 `\` 分隔符规范化）。
- 识别 `m_equip` 并按多源 path_family 阻止自动 retarget。
- 生成路径级 `RetargetPlan`（结构化分段替换，只改 slot 段）。
- 对多源 slot、未知 target、危险路径给出明确错误。

### 阶段 3：staging 与 InstallPlan 集成（AR3，已实现）

- 已通过 batch materializer 在 sibling `.partial` 中生成 staging，完整成功后原子发布，失败清理。
- `InstallPlan` 以 staging 的最终 target 为冲突键，同时保留原 `PackageFileId` provenance。
- manifest 记录 Mod/profile/revision-owned replacement binding snapshot；旧 JSON 缺字段时默认空。
- plan/token hash、manifest merge/uninstall/rollback 与真正重装 recovery 都消费同一 snapshot 事实。

### 阶段 4：Tauri contract 与前端工作流（AR4，已实现）

- 在 Mod 详情统一面板启用“替换目标”Tab，并由右键“MOD 文件修改”直达。
- 展示源槽位、target catalog、冲突预览和 warning。
- 首次安装只提交后端定义的 target/binding 选择，不让前端拼接路径。

### 阶段 5：切换目标、卸载与 Gate B（AR5，planned）

- 已安装 Mod 切换 target 必须复用 Gate A 真正重装，原子替换旧 entry/binding facts。
- 重启后恢复 target/安装事实，最终卸载恢复 ARMOR 安装前基线。
- 完成自动化、安全复审与 disposable Windows Sandbox 人工验收后再标记 Gate B `certified`。

### Gate B 后高级能力（planned + paused）

- 支持多源 slot 拆分。
- 支持整套/单部位策略。
- 评估二进制引用改写 transformer。
- 扩展到武器、语音或其他游戏。

## 开放问题

> 已决：第一版对 `m_equip`（男体路径）的处理是**识别并明确拒绝**——含 `m_equip` 的包按多源 path_family 处理（警告 + 阻止），不悄悄只处理 `f_equip`。等路径级 retarget 稳定后再评估是否支持男体重定向。

- armor catalog 的中文名称应以哪一版游戏文本为准，是否需要繁简差异。
- 部位缺失时是允许安装并提示，还是阻止 retarget。
- 外观 Mod 若包含 loose files 以外的说明文件、预览图或工具文件，应由包分析器如何分类。
- 玩家自定义 catalog 是否进入 MVP。

这些问题不阻塞第一版路径级 retarget，但应在实施计划前逐一确认。
