# Mod 安装方案规划

## 目标与边界

本文档规划 Helsincy Mod Manager 的 Mod 安装方案，重点解决“如何把 Mod 内容安全、可追踪、可回滚地作用到游戏目录”。

方案目标：

- 保留玩家原始游戏文件和存档安全边界。
- 所有安装结果都能通过 `InstallManifest` 追踪。
- 所有写入、覆盖、删除都必须来自 `InstallPlan`。
- 同一套安装计划可以适配不同提交后端。
- MVP 优先交付稳定可测能力，后续再扩展虚拟映射。

本文档不规划真实 Mod 包格式细节、不写死本地游戏路径、不把具体游戏规则塞进通用核心逻辑。游戏差异仍由对应 `GameAdapter` 和数据 catalog 表达。

## 总体策略

推荐采用“文件层栈 + 安装后端”的双层模型。

```text
导入包
  -> 安全检查
  -> 沙盒解压
  -> 包结构分析
  -> 文件层栈解析
  -> 生成 InstallPlan
  -> 选择 InstallBackend 提交
  -> 写入 InstallManifest
```

文件层栈负责回答“某个目标路径最终应该由哪个 Mod 文件提供”。安装后端负责回答“如何把这个结果应用到游戏目录”。这样可以把冲突分析、Profile、替换目标映射和安装提交方式解耦。

MVP 阶段推荐的实际提交方式是安全物理复制：从只读导入缓存或 staging 目录复制到游戏目录，覆盖前备份，完成后写 manifest。虚拟映射不作为 MVP 默认能力，而是作为后续可选后端接入同一套计划与清单。

## 核心概念：文件层栈

文件层栈是按最终目标路径聚合的逻辑视图。它不直接写游戏目录，而是为安装计划提供确定性的输入。

```text
FileLayerStack
  game_id
  game_instance_id
  profile_id
  target_path
  providers: [ModFileProvider]
  active_provider
  fallback
```

`target_path` 是相对游戏目录的规范化路径，例如 `nativePC/...` 或 adapter 允许的根目录文件。路径必须已经通过安全校验，不允许绝对路径、路径穿越或大小写碰撞。

`providers` 表示多个启用 Mod 对同一个目标路径的候选提供者。排序依据来自 profile 中的启用状态、优先级、安装时间、显式覆盖规则和游戏 adapter 规则。

`active_provider` 是当前计划中胜出的文件来源。若存在多个候选但没有明确优先级，必须输出冲突，而不是静默覆盖。

`fallback` 表示没有 Mod 生效时的状态。它可以是原游戏文件、无文件，或由 manifest 记录的上一状态。

## ModFileProvider

`ModFileProvider` 描述一个 Mod 如何提供某个目标文件。

```text
ModFileProvider
  mod_id
  package_file_id
  source_ref
  source_hash
  target_path
  priority
  install_kind
  replacement_binding_id
  generated_from
```

`source_ref` 指向只读导入缓存或 staging 产物。原始导入包只读保存；任何替换目标改写、重定向或格式修正都必须生成新的 staging 产物。

`install_kind` 用于区分提交后端需要的来源类型：

- `copy`：普通文件复制，MVP 默认。
- `generated`：由 retarget 或转换流程生成的 staging 文件。
- `link`：后续虚拟映射或链接后端使用。

`replacement_binding_id` 只在玩家选择外观、武器、语音等替换目标时出现。冲突检测必须基于绑定后的最终目标路径，而不是原始压缩包路径。

## 生命周期

### 1. Import

导入阶段只处理第三方压缩包，不触碰游戏目录。

必须完成：

- 检查压缩包类型、总大小、解压后大小和文件数量。
- 拒绝路径穿越、绝对路径、可疑链接和大小写碰撞。
- 解压到沙盒缓存目录。
- 记录导入包引用、文件列表、hash 和基础元数据。

导入结果是只读资产，后续安装不能直接修改导入缓存。

### 2. Analyze

分析阶段由通用分析器和游戏 adapter 协作完成。

通用分析器识别：

- 文件列表。
- readme / 配置文件。
- 预览图候选。
- DLL / exe / 脚本等高风险类型。

游戏 adapter 识别：

- 允许安装到哪些目标根。
- `nativePC` 等游戏资源目录结构。
- 前置依赖规则。
- 替换目标 catalog。
- 是否需要 retarget staging。

分析输出结构化数据，不把安装规则交给前端拼路径。

### 3. Resolve

解析阶段把启用的 Mod、Profile、替换目标绑定和 adapter 规则合并成文件层栈。

必须完成：

- 计算每个目标路径的候选 provider。
- 识别同一路径多 provider 冲突。
- 应用玩家显式优先级。
- 应用替换目标绑定后的目标路径。
- 输出依赖检查结果。

Resolve 阶段只产出逻辑视图，不做真实写入。

### 4. Plan

计划阶段把文件层栈转换为 `InstallPlan`。

MVP 动作集合建议为：

```text
ValidateGameInstance
ValidateCurrentState
CreateDirectory
BackupExistingFile
CopyFile
RemoveFile
WriteManifest
```

后续动作可以扩展为：

```text
CreateLink
RemoveLink
ValidateLinkTarget
RepairLink
```

计划必须包含：

- 目标相对路径。
- 来源文件引用和 hash。
- 是否覆盖已有文件。
- 备份需求。
- 冲突和依赖检查结果。
- 预计工作量，用于任务进度。

如果计划仍存在阻断级冲突或缺失必需前置，不进入提交阶段。

### 5. Commit

提交阶段是真正修改游戏目录的阶段，必须短、串行、可恢复。

执行流程：

```text
获取 game_instance 写锁
重新校验游戏目录和当前文件状态
按计划创建目录
覆盖前写备份
复制或删除目标文件
写入 manifest
释放写锁
```

不要在持有写锁时做压缩包解压、全量 hash、复杂分析或图片解码。

### 6. Recover

恢复阶段处理安装中断、崩溃、强制关闭和部分失败。

启动或进入安装页时应扫描未完成任务：

- 发现 manifest 已写完且文件状态匹配：标记完成。
- 发现备份存在但目标文件半写入：尝试回滚。
- 发现文件已写但 manifest 未完成：标记为需要人工确认或执行恢复计划。
- 发现虚拟映射目标失效：后续由修复扫描处理。

恢复逻辑必须只依赖 manifest、任务日志、备份记录和可校验 hash，不能靠猜测目录内容。

## MVP 后端：安全物理复制

MVP 推荐只实现 `CopyInstallBackend`。

优点：

- 行为容易理解。
- 不依赖管理员权限或特殊文件系统能力。
- 容易在临时目录中测试。
- 失败恢复路径清晰。
- 与当前 `InstallPlan` / `InstallManifest` 设计吻合。

限制：

- 文件会真实写入游戏目录。
- 大量 Mod 切换 Profile 时可能需要复制和删除较多文件。
- Steam 校验或游戏更新可能覆盖已安装文件，需要后续修复扫描。

MVP 的关键是把物理复制做安全，而不是把压缩包直接解到游戏目录。所有写入必须经过沙盒、计划、备份、提交和 manifest。

## 后续后端：虚拟映射

虚拟映射可以作为后续可选能力，而不是替代 `InstallPlan`。

候选实现方式包括：

- 单文件符号链接。
- 目录 junction / symlink。
- 受控链接树。
- 更高级的文件系统映射层。

建议优先研究单文件或受控链接树，不把游戏资源根目录整体替换为链接目录。整体根目录映射风险更高，容易受权限、Steam 校验、游戏更新和第三方工具影响。

虚拟映射接入方式：

```text
FileLayerStack
  -> InstallPlan
  -> VirtualMappingBackend
  -> InstallManifest(backend = virtual_mapping)
```

虚拟映射后端仍必须记录：

- 链接目标。
- 链接类型。
- 来源 hash。
- 目标路径。
- 创建时间。
- 修复状态。
- 回滚方式。

主要风险：

- Windows symlink 权限和开发者模式差异。
- 链接目标被用户移动或删除。
- 游戏更新或 Steam 校验替换链接。
- 杀毒软件或安全软件拦截。
- 部分工具不跟随链接或错误处理 junction。
- 链接目录中的路径大小写碰撞更难排查。

因此虚拟映射应作为高级后端，通过配置或实验开关启用。默认后端仍保持安全物理复制，直到虚拟映射经过足够测试。

## InstallManifest 建议

Manifest 是卸载、回滚、修复和审计的唯一可信依据。

建议字段：

```text
InstallManifest
  manifest_id
  game_id
  game_instance_id
  profile_id
  mod_id
  backend
  status
  created_at
  completed_at
  files: [InstalledFile]
  backups: [BackupRecord]
  replacement_bindings: [ReplacementBindingSnapshot]
  plan_hash
```

```text
InstalledFile
  target_path
  source_ref
  source_hash
  installed_hash
  install_kind
  previous_state
```

```text
BackupRecord
  target_path
  backup_ref
  original_hash
  original_size
  created_at
```

`status` 至少应区分：

- `planned`
- `committing`
- `completed`
- `rollback_required`
- `rolled_back`
- `repair_required`

卸载必须基于 manifest 删除或恢复文件，不允许根据当前 Mod 包重新推测安装过什么。

## 模块边界

建议落点如下。

`hmm-core`：

- `FileLayerStack`
- `ModFileProvider`
- `InstallPlan`
- `InstallAction`
- `InstallManifest`
- `Conflict`
- `DependencyResult`
- 不接触真实文件系统。

`hmm-ports`：

- `InstallBackend`
- `ManifestRepository`
- `BackupRepository`
- `FileStateProbe`
- `ArchiveInspector`
- `TaskEventSink`

`hmm-app`：

- `ImportModUseCase`
- `AnalyzeModUseCase`
- `BuildInstallPlanUseCase`
- `CommitInstallPlanUseCase`
- `RollbackInstallUseCase`
- `RecoverInstallTasksUseCase`
- 负责任务编排、锁边界和事务边界。

`hmm-infra`：

- 压缩包检查和沙盒解压。
- 文件复制、删除、备份、hash。
- SQLite manifest 存储。
- Audit Log 写入。
- 后续链接或虚拟映射实现。

`hmm-games-mhw`：

- 游戏目录识别规则。
- 允许的安装根和文件规则。
- `nativePC` 结构识别。
- 前置依赖规则。
- 替换目标 catalog 和 retarget 规则。

前端：

- 展示导入结果、冲突、依赖、安装计划摘要和任务进度。
- 允许玩家选择 Profile、优先级和替换目标。
- 不直接拼接游戏路径。
- 不直接决定文件覆盖规则。

Tauri command：

- 参数解析。
- DTO 转换。
- 调用应用层用例。
- 订阅并转发任务事件。
- 不直接执行文件系统写入规则。

## 并发与锁

并发原则保持现有架构约束：

- 扫描、解压、hash、分析、冲突计算可以并行。
- 同一个 `game_instance_id` 的提交必须串行。
- 同一个 `profile_id` 的启用、禁用、批量切换必须串行。
- 数据库事务保持短边界。
- 进度事件必须携带 `task_id`。

建议任务拆分：

```text
ImportTask
  只处理压缩包检查和沙盒解压

AnalyzeTask
  只处理结构分析、预览图、依赖候选和 adapter 识别

PlanTask
  只处理文件层栈、冲突和 InstallPlan

CommitTask
  获取写锁并执行 InstallBackend

RecoverTask
  启动时或手动触发，修复未完成状态
```

## 安全策略

安装方案必须默认不信任 Mod 包和外部路径。

必须拒绝：

- 路径穿越。
- 绝对路径。
- 指向沙盒外的链接。
- 解压后超过限制的压缩包。
- 大小写不敏感平台上的目标路径碰撞。
- adapter 不允许的根目录写入。

必须记录：

- 每次高风险文件操作的脱敏审计日志。
- 备份引用和 hash。
- 安装前后状态。
- manifest 状态变更。

日志和诊断包不能包含完整本地路径、真实玩家存档内容、第三方 Mod 包内容、token、cookie 或可还原隐私的数据。

## 分阶段路线

### 阶段 A：复制后端 MVP

- 建立 `FileLayerStack` 和 `ModFileProvider` 领域模型。
- 扩展 `InstallPlan` 动作类型。
- 实现基于最终目标路径的冲突检测。
- 实现 `CopyInstallBackend`。
- 写入 `InstallManifest`。
- 支持基于 manifest 卸载。
- 覆盖中途失败回滚。

### 阶段 B：Profile 与批量切换

- 将启用状态和优先级纳入文件层栈。
- 支持同一 profile 串行启用、禁用和批量应用。
- 支持多个 Mod 写入同一路径时的可解释冲突。
- 为 UI 提供计划预览摘要。

### 阶段 C：替换目标与 retarget staging

- 接入替换目标 catalog。
- 将玩家绑定关系转为 provider 的最终目标路径。
- 只在 staging 目录生成改写产物。
- manifest 记录绑定快照。
- 切换目标时执行“卸载旧绑定 + 安装新绑定”。

### 阶段 D：修复扫描

- 扫描 manifest 与游戏目录状态差异。
- 检测文件缺失、hash 不符、备份缺失和未完成任务。
- 提供重新安装、回滚、忽略三类处理。
- 为后续虚拟映射提供链接修复基础。

### 阶段 E：虚拟映射实验后端

- 新增 `VirtualMappingBackend`。
- 优先支持单文件或受控链接树。
- manifest 记录链接类型和链接目标。
- 增加权限探测和后端可用性检查。
- 默认关闭，通过实验配置启用。
- 保留复制后端作为稳定 fallback。

## 测试矩阵

导入与路径安全：

- 正常 zip / 7z 样本。
- 含 `nativePC` 的样本。
- 根目录 DLL 样本。
- 路径穿越样本。
- 绝对路径样本。
- 大小写碰撞样本。
- 伪装图片样本。
- 超出大小或文件数量限制的样本。

计划与冲突：

- 单 Mod 新增文件。
- 单 Mod 覆盖已有文件。
- 两个 Mod 写入同一路径。
- 显式优先级解决冲突。
- 缺失必需前置阻断安装。
- 替换目标绑定后目标路径冲突。

复制后端：

- 新文件安装。
- 覆盖前备份。
- 安装中途失败并回滚。
- manifest 写入失败时不留下半完成状态。
- 基于 manifest 卸载。
- 游戏目录状态变化后的提交前重校验。

恢复：

- `committing` 状态启动恢复。
- 文件已复制但 manifest 未完成。
- manifest 完成但文件 hash 不匹配。
- 备份缺失时进入人工处理状态。

虚拟映射后续测试：

- symlink 权限不可用。
- 链接目标缺失。
- 链接被游戏更新替换为普通文件。
- 卸载时只移除本工具创建的链接。
- fallback 到复制后端。

所有测试默认使用临时目录，不读写真实游戏目录、真实玩家存档或真实第三方 Mod 包。

## 结论

适合本项目的安装方案不是在“直接覆盖”和“虚拟映射”之间二选一，而是先抽象出稳定的文件层栈，再通过安装后端提交结果。

MVP 应优先实现安全物理复制后端，因为它最容易验证备份、回滚、manifest 和审计链路。虚拟映射保留为后续高级后端，在同一套 `InstallPlan` 和 `InstallManifest` 之下渐进接入。这样既能快速交付可靠的安装能力，也能为 Profile、批量切换、替换目标映射和未来虚拟映射留下清晰扩展点。
