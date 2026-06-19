# MHW:I 外观套装重定向设计

## 背景

很多《怪物猎人：世界 冰原》外观 Mod 并不是新增一套独立外观，而是把自定义模型、贴图或材质覆盖到官方装备槽位。玩家真正关心的工作流是：

```text
导入一个外观 Mod
选择它要替换哪套官方装备或幻化
安装后进入游戏装备该官方套装时看到 Mod 效果
```

例如一个 Mod 作者把“红色礼服裙”打包为替换守护者套装，玩家希望它改为替换黑龙套装。管理器需要把这个“Mod 资源 -> 官方套装槽位”的绑定关系建模为一等概念，而不是把它当成一次性的文件改名工具。

前期调研验证了 MHW:I 常见外观包的基础机制：把包内 `nativePC/pl/f_equip/<plNNN_VVVV>/...` 路径中的套装编号替换为目标套装编号，再输出可安装内容。类似脚本级做法可以帮助理解资源槽位规则，但缺少压缩包安全、manifest、备份、回滚、日志脱敏和跨平台路径规范化，不能作为运行时依赖直接接入本项目。

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
  id
  game_id
  target_type
  display_name
  aliases
  internal_id
  path_family
  part
  is_full_body
  metadata

ReplacementBinding
  id
  mod_id
  profile_id
  source_asset
  target_id
  created_at

RetargetPlan
  binding
  actions
  warnings

RetargetAction
  action_type
  source_relative_path
  staged_relative_path
  source_slot
  target_slot
```

`metadata` 可以承载游戏适配器专属信息，但核心层只把它当作结构化数据，不解析 MHW:I 语义。

### `hmm-ports`

负责声明应用层依赖的 trait。

建议能力：

```text
GameAdapter
  replacement_catalog(game_id) -> Vec<ReplacementTarget>
  analyze_replacement_assets(package) -> ReplacementAnalysis
  build_retarget_plan(request) -> RetargetPlan

StagingFileSystem
  copy_to_staging(source, destination)
  list_staged_files(staging_id)
```

具体签名应跟现有 crate 风格保持一致。`GameAdapter` 是否拆分成更小的 `ReplacementAdapter` 可以在实现计划阶段决定。

### `hmm-games-mhw`

负责 MHW:I 专属规则：

- armor catalog。
- `pl/f_equip/<slot>` 路径族识别。
- `plNNN_VVVV` 编号解析与校验。
- 源槽位推断。
- 路径级 `RetargetPlan` 生成。
- 黑龙/煌黑龙等名称和别名区分。

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

示例：

```json
{
  "id": "mhw:armor:fatalis-alpha",
  "game_id": "mhw",
  "target_type": "armor",
  "display_name_zh_cn": "【精英‧龙α】服装",
  "display_name_en": "Fatalis Alpha +",
  "aliases": ["黑龙α", "黑龙 Alpha", "Fatalis α"],
  "internal_id": "pl129_0000",
  "path_family": "pl/f_equip",
  "rank": "master",
  "variant": "alpha",
  "is_full_body": false,
  "parts": ["head", "body", "arms", "waist", "legs"]
}
```

黑龙相关目标必须显式区分：

```text
【精英‧龙α】服装     -> Fatalis / 黑龙α     -> pl129_0000
【精英‧龙β】服装     -> Fatalis / 黑龙β     -> pl129_0010
【精英·煌黑龙α】服装 -> Alatreon / 煌黑龙α -> pl052_0000
【精英·煌黑龙β】服装 -> Alatreon / 煌黑龙β -> pl052_0010
```

UI 搜索“黑龙”时应同时展示清晰的怪物名、套装名和内部编号，避免玩家误选。

## 包分析

包分析器应在安全解压后的 sandbox/cache 中分析相对路径。第一版只识别规范化后的路径：

```text
nativePC/pl/f_equip/<slot>/...
```

其中 `<slot>` 必须匹配：

```text
pl[0-9]{3}_[0-9]{4}
```

分析输出建议：

```text
ReplacementAnalysis
  detected_targets
  source_slots
  supported_retarget_kinds
  warnings
```

对于多个源 slot：

- 若全部属于同一 slot，允许生成 retarget。
- 若发现多个 armor slot，第一版给出警告并阻止自动 retarget。
- 后续可以支持多 binding 或高级拆分。

## RetargetPlan 生成

路径级 retarget 的规则：

```text
source: nativePC/pl/f_equip/pl121_0000/arm/mod/f_body.mod3
target: nativePC/pl/f_equip/pl129_0000/arm/mod/f_body.mod3
```

生成 plan 时需要：

- 校验 source slot 来自包分析结果。
- 校验 target id 存在于当前游戏 catalog。
- 校验 source 和 target path 都是相对路径。
- 只对 slot 段进行结构化替换，不做任意字符串全局替换。
- 保留原始文件内容，只改变 staging 目标路径。
- 记录 warning，例如目标和源相同、多个源 slot、缺失常见部位。

不要对整条路径做宽泛字符串替换，因为这会误改文件名、父目录或其他碰巧包含同样数字的片段。

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

## Manifest 与审计

安装 manifest 必须记录 replacement 信息：

```text
replacement_bindings
  binding_id
  source_slot
  target_id
  target_slot
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

- catalog 中每个 `internal_id` 符合 `plNNN_VVVV`。
- `黑龙` 和 `煌黑龙` alias 不混淆。
- path parser 能识别 `/` 和 `\` 输入并规范化。
- 多源 slot 返回 `AmbiguousSourceSlot`。
- 未知 target id 返回 `TargetCatalogMissing`。
- retarget action 只替换 slot 段，不改其他路径片段。

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

### 阶段 1：模型与 catalog

- 定义 replacement/retarget 领域模型。
- 为 MHW:I 建立 armor catalog。
- 提供 catalog 查询和基础校验测试。

### 阶段 2：包分析与路径级 RetargetPlan

- 分析 `nativePC/pl/f_equip/<slot>`。
- 生成路径级 `RetargetPlan`。
- 对多源 slot、未知 target、危险路径给出明确错误。

### 阶段 3：staging 与 InstallPlan 集成

- materialize staging。
- 让 `InstallPlan` 以 staging 的最终路径为输入。
- manifest 记录 replacement binding。
- 冲突检测基于最终路径。

### 阶段 4：前端工作流

- 启用 `替换目标` 页面或在 Mod 安装流程中加入目标选择。
- 展示源槽位、target catalog、冲突预览和 warning。
- 提供切换目标的卸载重装流程入口。

### 阶段 5：高级能力

- 支持多源 slot 拆分。
- 支持整套/单部位策略。
- 评估二进制引用改写 transformer。
- 扩展到武器、语音或其他游戏。

## 开放问题

- 是否需要第一版同时支持男性装备路径 `m_equip` 和女性装备路径 `f_equip`，还是先只支持 `f_equip`。
- armor catalog 的中文名称应以哪一版游戏文本为准，是否需要繁简差异。
- 部位缺失时是允许安装并提示，还是阻止 retarget。
- 外观 Mod 若包含 loose files 以外的说明文件、预览图或工具文件，应由包分析器如何分类。
- 玩家自定义 catalog 是否进入 MVP。

这些问题不阻塞第一版路径级 retarget，但应在实施计划前逐一确认。
