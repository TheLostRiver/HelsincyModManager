# 安装恢复受控动作实施计划

本文档细化 `InstallPlan` 后续“自动回滚/恢复执行、`rollback_required` rich 状态和真正的受控恢复/回滚动作”的实施边界。当前只读恢复扫描、恢复中心、Dashboard 摘要和 App Frame 全局告警已经落地，但它们只展示状态，不执行恢复、删除、回滚或 manifest 写入。

本计划的目标是把后续写入动作拆成可审查、可测试的小切片，避免在状态事实不足时直接对游戏目录执行破坏性操作。

## 当前事实

已可依赖：

- `scan_install_recovery({ gameId, profileId, modIds })` 是只读扫描入口，能基于 manifest、目标文件摘要和 backup 可读性返回 `completed`、`repair_required`、`unknown` 或 `not_installed`。
- `modIds: []` 会扫描当前 profile manifest 内全部已知托管 Mod，可支撑启动级提示和恢复中心聚合。
- `start_install_task` 和 `start_uninstall_task` 都复用 `gameId/profileId` 写锁；commit / uninstall 写入窗口串行。
- `UninstallModService` 已有最小安全删除/恢复能力：只在 `installed_file` 摘要匹配且 backup 可读时删除新增文件或恢复覆盖文件。
- 安装和卸载任务已写最小 Audit Log，字段只包含 task/game/profile/mod id 和聚合计数，不记录完整路径或 backup ref。

当前不能假设：

- MVP manifest 还没有持久化 `planned`、`committing`、`rollback_required`、`rolled_back` 等 rich 状态。
- 安装进程在写入文件但尚未保存 manifest 时崩溃，当前系统不能仅凭目录内容安全推断“本工具写过哪些目标”。
- Task Log / Audit Log 不能成为唯一事实来源；它们只能辅助诊断，不能替代 manifest、backup 和受控目标摘要。
- `repair_required` 不等于“可以自动覆盖或删除”。目标被外部工具或玩家修改时，自动恢复可能误伤玩家文件。

## 安全原则

- 恢复动作必须由用户主动触发，MVP 不做后台自动恢复。
- 所有写入、删除、恢复和 manifest 变更必须在同一 `gameId/profileId` 写锁下执行，并在写入前重新验证目标状态。
- 可执行动作只来自后端生成的恢复计划或恢复任务，不接受前端传入 target path、backup ref、manifest path、sandbox/cache path 或本地路径。
- 恢复动作只能基于 manifest、backup、受控 recovery record 和当前目标摘要；不能基于当前 Mod 包内容重新猜测。
- 对目标缺失、目标摘要变化、backup 缺失、backup 读取失败或旧 manifest 缺少摘要的场景，默认阻断自动动作并转入人工处理。
- 任何成功、失败、回滚成功或回滚失败都必须写 Audit Log，且只记录短 id、计数和稳定错误分类。
- 诊断导出可以辅助用户反馈，但不能把诊断包当作恢复状态来源。

## 目标状态模型

后续 rich recovery 状态应至少覆盖：

| 状态 | 含义 | 自动动作 |
| --- | --- | --- |
| `completed` | manifest、目标文件摘要和必要 backup 均一致。 | 可提供安全卸载；恢复中心可显示无需处理。 |
| `rollback_required` | 后端有持久化证据表明 commit 已进入写入窗口且未完成，需要回滚到安装前状态。 | 仅当所有目标仍匹配可回滚前置条件时，提供受控回滚任务。 |
| `rolled_back` | 已执行回滚，游戏目录恢复到安装前状态或清理了本工具新增文件。 | 不再重复回滚；保留审计记录。 |
| `repair_required` | manifest、backup 或目标状态不一致，但无法安全自动判断。 | 阻断破坏性操作，仅提供诊断和人工处理建议。 |
| `unknown` | 目标或 backup 读取失败，状态不可判定。 | 阻断自动动作，提示重新扫描或导出诊断。 |

`rollback_required` 不能只由当前目标文件变化推断，必须来自未来的 durable recovery record、rich manifest status 或等价受控事务记录。

## 恢复动作集合

### `preview_recovery_action`

只读预览，不写文件、不写 manifest、不写 backup。

输入只允许：

- `gameId`
- `profileId`
- `modId`
- `actionKind`

输出只允许：

- 稳定 action id 或 action kind。
- 可执行性：`available` / `blocked`。
- 聚合计数：将删除的新文件数、将恢复的覆盖文件数、需要 backup 的文件数、阻断 issue 计数。
- 稳定阻断 reason code，例如 `target_changed`、`target_missing`、`backup_missing`、`missing_installed_file_summary`、`rollback_state_missing`。

禁止输出：

- target path。
- backup ref/root。
- manifest path/root。
- sandbox/cache path。
- 目标 hash 明文。
- manifest 正文或第三方 Mod 内容。

### `rollback_install`

受控回滚任务，用于把一个有充分状态事实的托管安装恢复到安装前状态。

可执行前置：

- 后端能定位同一 `gameId/profileId/modId` 的 manifest 或 durable recovery record。
- 每个候选 entry 都有 `installed_file` 摘要。
- 当前目标文件仍与 `installed_file` 摘要匹配。
- 对覆盖文件，backup 必须存在且可读。
- 对新增文件，目标仍必须是本工具写入的 installed 摘要，才能删除。
- 写入前在持锁区内重新读取目标和 backup，不能只相信预览结果。

执行结果：

- 新增文件：删除目标文件。
- 覆盖文件：从 backup 恢复原始文件。
- manifest / recovery record：记录 `rolled_back` 或移除已回滚 entry，具体行为由 rich manifest 切片定稿。
- Audit Log：记录 task id、game id、profile id、mod id、removed/restored 文件计数、结果和稳定错误分类。

阻断场景：

- 目标缺失。
- 目标摘要不匹配。
- backup 缺失或读取失败。
- manifest 缺少 `installed_file`。
- recovery record 缺少可证明的写入事实。
- 任一目标在预览后、执行前发生变化。

### `verify_and_mark_completed`

后续可选动作，用于“持久状态未完成，但目标文件和 manifest 事实可证明安装已完成”的场景。

MVP 不先实现该动作。原因是它会写 manifest 或 recovery record，但不恢复文件；如果状态事实设计不清，会把半完成状态误标为完成。

### `manual_resolved`

后续可选动作，用于玩家手工修复后让工具重新扫描并确认状态。

MVP 不先实现“手动标记已处理”。任何手动处理都必须通过重新扫描得到 `completed` 或继续保持 `repair_required` / `unknown`，不能由前端按钮直接改 manifest 状态。

## 推荐实施切片

### 切片 1：durable recovery record / rich status 基础

目标：让系统有足够事实表示 commit 是否进入写入窗口、哪些目标已经应用、失败后是否需要回滚。

候选落点：

- `hmm-core`：新增 game-independent recovery status / action 类型，或扩展 manifest rich 状态模型。
- `hmm-app`：在安装 commit 编排中定义可测试的状态转换边界。
- `hmm-ports`：如果需要持久化 recovery record，新增窄 repository trait。
- `hmm-infra`：使用 app data 下受控 JSON/SQLite 存储；禁止写仓库、游戏目录或临时无命名空间目录。

验收：

- 旧 manifest 兼容读取。
- 没有 durable recovery record 时，扫描不能生成 `rollback_required`。
- 状态转换测试覆盖 `planned -> committing -> completed`、`committing -> rollback_required`、`rollback_required -> rolled_back`。
- 不新增前端按钮或自动写入动作。

### 切片 2：只读恢复动作预览

目标：恢复中心可以显示“是否有可执行受控回滚动作”，但仍不执行。

候选落点：

- `hmm-app`：新增 `InstallRecoveryActionPreviewService`，依赖 manifest/recovery record、game filesystem、backup store。
- `src-tauri/src/install_commands.rs`：新增窄 command 时必须更新 `FRONTEND_BACKEND_CONTRACT.md`。
- `src/features/install-recovery/`：只展示后端 preview 摘要，不拼接路径。

验收：

- command 不接受路径或 backup ref。
- preview 对 `target_changed`、`target_missing`、`backup_missing`、`unknown` 全部阻断。
- preview 只返回聚合计数和稳定 reason code。
- 前端在 blocked 时只提供重新扫描和诊断导出。

### 切片 3：受控回滚任务

目标：用户主动触发后，后端在写锁下执行 `rollback_install`，并发出可追踪任务事件。

候选 phase code：

- `install.recovery.queued`
- `install.recovery.planning`
- `install.recovery.processing`
- `install.recovery.completed`
- `install.recovery.failed`

验收：

- 任务事件全部携带 `taskId`。
- 同一 `gameId/profileId` 与 install/uninstall 共用写锁。
- 预览通过后，执行前仍在持锁区重新验证目标摘要和 backup。
- 删除/恢复/manifest 保存任一步失败时，执行 best-effort rollback，并写 Audit Log。
- 测试使用临时目录或 fake filesystem，不依赖真实 MHW 安装目录。

### 切片 4：恢复中心 UI 启用

目标：只有后端 preview 显示 `available` 时，恢复中心才显示受控回滚按钮。

验收：

- 前端只提交 `gameId`、`profileId`、`modId` 和后端稳定 action id / action kind。
- 任务状态按 `taskId` 匹配。
- UI 不展示 target path、backup ref、manifest path、hash、日志正文或第三方 Mod 内容。
- 操作完成后重新调用只读 `scan_install_recovery`，不依赖内存任务态当作最终事实。

## 测试矩阵

| 场景 | 必须覆盖 |
| --- | --- |
| 无 recovery record | 不能生成 `rollback_required`，不能提供受控回滚动作。 |
| completed 且目标匹配 | 可预览安全卸载/回滚等价摘要，但不应绕过已有卸载边界。 |
| 新增文件目标匹配 | 回滚任务可删除该目标，并更新 manifest/recovery record。 |
| 覆盖文件目标匹配且 backup 可读 | 回滚任务可恢复 backup。 |
| 目标缺失 | 阻断自动动作，返回稳定 reason code。 |
| 目标摘要变化 | 阻断自动动作，避免覆盖外部修改。 |
| backup 缺失或读取失败 | 阻断自动动作。 |
| manifest 保存失败 | 已执行文件动作必须 best-effort rollback，并写失败审计。 |
| 写入/删除失败 | 保留可恢复状态，返回 failed task event 和 Audit Log。 |
| 并发 | install、uninstall、recovery action 对同一 game/profile 串行。 |
| 脱敏 | command error、task event、Audit Log 和诊断摘要不含完整路径、backup ref/root 或 manifest 正文。 |

## 性能边界

- 预览和执行只扫描目标 `modId` 相关 manifest entries，不做全游戏目录扫描。
- 只对候选目标读取字节并计算摘要；不在 UI 层或前端做 hash。
- 执行任务在写锁内只做短 revalidate、delete/restore、manifest/recovery record 写入；复杂分析、全量扫描和诊断导出保持在写锁外。
- 诊断导出保持用户主动触发，不作为恢复动作前置。

## 文档同步要求

任一后续切片新增或修改 command、DTO、task phase、错误码、manifest 字段或 Audit Log 语义时，必须同步检查：

- `docs/INSTALL_PLAN_STATUS.md`
- `docs/INSTALL_PLAN_MVP_TODO.md`
- `docs/FRONTEND_BACKEND_CONTRACT.md`
- `docs/LOGGING.md`
- `docs/TESTING.md`

如果仅实现内部 rich status 而不暴露 command，仍需更新 InstallPlan 状态/TODO，说明哪些恢复动作仍不可用。

## 禁止捷径

- 不新增 `restore_file`、`delete_path`、`write_manifest` 等宽泛 command。
- 不让前端提交 target path、backup ref、manifest path 或任意本地路径。
- 不把 `repair_required` 直接当作“可以自动修复”。
- 不用 Task Log / Audit Log 代替 manifest 或 recovery record。
- 不覆盖摘要不匹配的目标文件。
- 不自动删除 manifest 未记录的未知文件。
- 不为了通过 UI 流程而跳过 Audit Log 或写锁。
