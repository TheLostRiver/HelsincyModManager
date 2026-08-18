# SAVE-05 Retention 与备份中心设计

## 状态

- 路线图任务：SAVE-05
- 实施状态：`certified`（2026-08-16）
- 前置：SAVE-04 玩家存档恢复已 `certified`
- 平台范围：Windows + MHW:I

本文定义存档备份按数量、时间和空间治理的安全语义，以及独立备份中心的后端权威查询、备注和受控恢复入口。
自动化和人工验收只使用 temp/artificial fixture，不读取真实 Steam userdata、真实游戏目录或真实玩家存档。

## 目标

- 让现有 `maxAgeDays` 设置真正生效，并增加可选的 Profile 总空间预算。
- 保持普通 retention 永久排除 SAVE-04 的 `pre_restore` 安全保护点。
- 用持久化意图和结构化报告表达完成、部分清理、阻断和失败，不把半删状态伪装成成功。
- 建立独立 `features/backups/` 页面，跨 Profile 展示历史、状态、空间摘要、确认过的 Steam 展示摘要、备注和受控恢复入口。
- 复用 SAVE-04 的 `preview_save_restore` / `start_save_restore_task`，不增加路径型恢复命令。

## 非目标

- 不读取或上传存档内容。
- 不把完整路径、Steam ID、archive hash、manifest 正文或文件列表暴露给前端。
- 不让前端计算 retention、空间占用、ownership 或恢复可用性。
- 不让普通 retention 删除 `pre_restore`；其未来独立策略必须另行设计和确认。
- 不引入 Production CLI 写能力，也不扩展到 Linux / Steam Deck。

## 配置模型

`ProfileBackupRetention` 增加：

```text
max_count: u32
max_age_days: Option<u32>
max_total_bytes: Option<u64>
```

- `max_count` 范围为 0..=999；`0` 表示关闭按数量治理。
- `max_age_days = None` 表示关闭按年龄治理；有值时范围为 1..=3650 天。
- `max_total_bytes = None` 表示关闭空间预算。为避免升级后静默删除既有备份，migration 和默认值都使用 `None`。
- 启用时范围为 16 MiB..=1 TiB。UI 使用 GiB/MiB 展示，Tauri DTO 仍传精确字节数。
- UI 对数量、年龄和空间显示 `0 = 不限制`。年龄与空间提交时把 0 归一化为 DTO `null`；Tauri 边界
  也兼容将数值 0 归一化为领域层 `None`。新配置档和用户主动重置时三项默认均不限制；既有 Profile
  设置不迁移、不覆盖。
- 空间预算作用于同一 `gameId/profileId` 的 HMM 已知备份 archive 总量；manifest 是受控元数据，但不计入预算数值。

Profile 存档设置同时增加后端确认来源的可选展示快照：

```text
SteamAccountDisplaySummary
  account_name: Option<String>
  avatar_url: Option<String>
  account_label: String
```

该快照只用于展示，不参与目录 ownership、retention 或 restore 校验。候选确认或唯一高置信候选自动写入时保存；
用户把存档目录改为不同目录时清空，相同目录只修改 schedule/retention 时保留。

## Retention 候选与优先级

### 分类

- 普通候选：`status = completed` 且 trigger 为 `manual | auto | pre_install`。
- 保护点：trigger 为 `pre_restore`。计入总空间事实，但普通 count/age/space retention 不得删除。
- 可恢复清理：`retention_pending | retention_partial`。下一次维护优先重试，不重新解释为普通 completed。
- 终态：`deleted_by_retention`，不计入占用。
- 问题项：`missing | invalid`，不自动删除；在报告和备份中心中单独显示。

### 规则合并

1. 按 `created_at DESC, backup_id DESC` 确定普通备份的新旧顺序。
2. `max_count` 非零时把第 N 个之后的普通备份列为候选；为 0 时不产生 count 候选。
3. `max_age_days` 把 `created_at < now - days * 86_400_000` 的普通备份列为候选。
4. count/age 候选先从空间模拟中移除；若剩余已知 archive 字节仍超过预算，再按最旧优先增加普通候选。
5. 最新一份普通 completed 备份始终保留。若它或受保护 `pre_restore` 已使预算不可收敛，返回 `blocked`，不得突破保护边界。
6. 同一备份可同时带 `count | age | space | retry` 原因；删除顺序固定为 `created_at ASC, backup_id ASC`。

普通 `manual | auto | pre_install` 新备份的 archive、manifest 和 SQLite 历史全部成功后才运行 retention。
`pre_restore` 创建只负责先落盘恢复前保护点，不在同一次调用中触发普通 retention，避免恢复来源在后续
重新校验前被清理；后续普通备份或显式“立即整理”再执行策略。清理失败不能反向把新备份判定为失败，
但必须在任务 warning、Audit 和备份中心最新报告中显式投影。

## 持久化删除协议

文件系统无法把 archive、manifest 和 SQLite 三方合并为单一原子事务，因此使用可重试事实链：

```text
Completed
  -> SQLite compare-and-set RetentionPending + reasons + attempted_at
  -> capability-relative/no-follow 复验 archive 与 manifest
  -> 分别删除，并返回结构化文件结果和实际释放字节
  -> 两项均已不存在：SQLite DeletedByRetention
  -> 任一项仍存在或不可证明：SQLite RetentionPartial + stable error
```

约束：

- 物理删除前必须先成功写入 `RetentionPending`；写入失败时不碰文件。
- writer 只接受 repository 中的目录快照和受控文件名，不接受前端路径。
- backup profile 目录、`pre-restore` 子目录和直接子文件均以 no-follow capability 打开。
- archive/manifest 必须是普通文件；link、junction、reparse、目录、未知类型或打开前后 identity 变化均 fail closed。
- 只有成功打开受控父目录后，直接子文件的 `NotFound` 才表示“已经不存在”；父目录缺失、离线或无法打开
  一律返回 `blocked + save_backup_retention_directory_unavailable`，不能把自定义备份根暂时不可用误报为
  已清理。结构化报告必须区分 `deleted | already_missing | blocked`。
- 若文件已删除但最终 SQLite 写回失败，记录仍停在 `retention_pending`；下一次维护根据缺失事实收敛为
  `deleted_by_retention`，不会再次释放或重复计算字节。
- `retention_partial` 不可用于 restore，且在备份中心显示“清理未完成”。

新增持久字段只保存稳定事实：原因位、最近尝试时间、稳定错误码和已确认释放的 archive 字节；不保存路径或底层错误原文。

## Retention 报告

```text
SaveBackupRetentionReport
  outcome: within_policy | completed | partial | blocked | failed
  evidence_degraded: bool
  scanned_count
  protected_count
  problem_count
  candidate_count
  deleted_count
  partial_count
  blocked_count
  archive_bytes_before
  archive_bytes_after
  released_bytes
  max_total_bytes
  budget_satisfied
```

- `within_policy`：无需删除且预算满足。
- `completed`：所有候选已收敛，预算满足。
- `partial`：至少一项删除未收敛，但其他项可继续处理。
- `blocked`：保护点、最新普通备份或问题项使预算无法安全收敛。
- `failed`：repository、clock 或顶层目录事实不可用，无法建立可信计划。

报告只包含计数、字节、布尔值和稳定 enum/code。evidence_degraded 仅表示本次清理完成后 Audit
写入未能确认，不改变已完成的文件清理结果，也不把业务成功重分类为失败。自动备份任务在对应
completed event 上携带稳定 code save_backup_evidence_degraded；显式维护返回完整 DTO 并在 UI 中
显示证据降级提示。

## Repository 与查询

`SaveBackupRepository` 增加后端权威能力：

- 按 Profile 读取全部 retention facts。
- compare-and-set 开始清理和写回清理结果。
- 按 `game/profile/backup` 更新规范化备注。
- 跨 Profile 分页查询和聚合，不让 React 对 Profile 列表做 N+1 查询。

备份中心查询使用固定上限分页：

```text
query_save_backup_center({
  gameId,
  profileId?,
  trigger?,
  status?,
  search?,
  offset,
  limit
})
```

- `limit` 范围 1..=100，默认 30。
- `offset` 必须能无损转换为 SQLite signed integer；异常大值返回稳定 query invalid，不允许 wrap 为负数。
- `search` 只匹配后端持久化的 Profile 名称和备注，最长 100 个 Unicode scalar；不搜索路径或 manifest。
- 聚合与页面来自同一次后端查询语义，返回总数、状态计数、已知 archive 字节和 Profile 摘要。

写命令：

```text
update_save_backup_note({ gameId, profileId, backupId, note? })
run_save_backup_retention({ gameId, profileId })
```

备注 trim 后最长 200 字符；请求只接收短 ID 和文本，不接收 archive/manifest/path。

## 并发

- `SaveProfileMaintenanceScopeRegistry` 按 `gameId/profileId` 串行化 queued/running backup、备份末尾的
  自动 retention、显式 `run_save_backup_retention` 和 restore。恢复任务从登记到 terminal 持有该 scope，
  retention 不能在来源校验、准备、pre-restore 保护点或提交之间删除同一普通备份。
- `SaveBackupTaskScopeRegistry` 与 `SaveRestoreTaskScopeRegistry` 在 runtime composition 中复用同一维护
  registry；冲突继续映射为各自稳定 task conflict。restore registry 仍单独维护退出 admission，普通备份
  或 retention 不会被误投影成 active restore。
- 维护 scope 不替代 game/profile 写锁。archive 校验、解压、staging 和 pre-restore 备份仍在游戏写锁外；
  只有目标目录交换、rollback 和 recovery 收尾进入既有短写锁。
- 扫描和计划不持有 SQLite 写事务；每个状态转换使用短 compare-and-set。
- 进入单个备份的持久化删除链后不接受取消，必须写回 completed/partial 或保留 pending 供重试。
- restore 只接受 `completed`；pending/partial/deleted/missing/invalid 均 fail closed。

## 备份中心 UI

启用 `/backups` 路由和“存档备份”导航项。页面是工作台式密集布局，不使用横向滚动表格。

首屏包含：

- 全局摘要：备份数、已知空间、保护点数、需处理数。
- Profile/trigger/status 筛选和备注搜索。
- Profile 摘要：名称、活动状态、确认过的昵称/头像或掩码 label、retention 和空间状态。
- 历史记录：备注或文件名、时间、大小、文件数、trigger、status。
- 每条 completed 记录直接显示“恢复存档”；调用 SAVE-04 preview/token/task，不自行构造路径。
- 备注使用显式编辑按钮和保存/取消状态。
- 每个 Profile 提供“立即整理”；执行前使用共享 `alertdialog` 二次确认并默认聚焦取消，确认文案明确普通
  备份可能被永久删除、最新普通备份和 `pre_restore` 保护点保留。确认后显示动态进度、耗时和结构化结果。
- 空间预算输入在前端按整数 MiB 钳制到 16 MiB..=1 TiB，服务端继续执行最终权威校验；搜索框限制 100 字符。
- 持久化 Steam 头像在渲染前再次要求无凭据、无自定义端口的 HTTPS URL，且 hostname 精确匹配受信 Steam
  头像域名；不满足时回退账号名或 Profile 名首字母。

页面必须覆盖 loading、empty、error、partial、blocked、reduced-motion 和 480px 窄宽状态。按钮尺寸稳定，
文本可换行，恢复入口不能被隐藏到横向滚动区域。

## Audit 与隐私

自动或显式 retention 记录 `retention_pruning` Audit。允许字段：

```text
task_id? game_id profile_id trigger operation result error_code
outcome scanned_count protected_count problem_count candidate_count
deleted_count partial_count blocked_count
archive_bytes_before archive_bytes_after released_bytes budget_satisfied
```

备注正文、Profile 名称、账号昵称、头像 URL、文件名、完整路径、Steam ID、manifest/hash 列表和底层错误原文均不得进入 Audit。

## 测试矩阵

后端聚焦测试至少覆盖：

- count、age、space 单独和组合规则的确定性排序与边界时间。
- 默认 `max_count = 0`、`max_age_days = None`、`max_total_bytes = None` 不产生任何新删除候选。
- 最新普通备份和所有 `pre_restore` 永不被普通 retention 删除。
- 保护点导致预算不可收敛时返回 blocked。
- begin intent 失败时没有文件删除。
- archive/manifest 单项缺失、单项删除失败、DB 最终写回失败和下次重试收敛。
- link/junction/reparse、目录替换和 identity 变化 fail closed，外部 sentinel 不受影响。
- 跨 Profile 分页、筛选、聚合和备注持久化。
- Steam 展示快照确认后持久化、相同路径保留、不同路径清空、旧数据库 migration 默认兼容。
- 显式维护与同 Profile backup scope 冲突。
- DTO/command 不含路径、Steam ID、manifest、hash 或任意文件操作参数。

前端聚焦测试至少覆盖：

- `/backups` 路由与导航启用。
- loading/empty/error/partial/blocked 文案和响应式无横向滚动。
- 筛选、分页、备注保存和整理反馈。
- “立即整理”二次确认、取消默认焦点、搜索上限、空间预算钳制和持久化头像 URL 的二次 fail-closed。
- completed 才能进入 SAVE-04 恢复 preview；其他状态按钮禁用。
- API wrapper command 名和 DTO camelCase 契约。

候选完成前运行完整 `scripts/verify.ps1`，随后执行 findings-first 全 diff 审查和 disposable Windows synthetic gate。

## Disposable Windows 验收

人工 fixture 至少包含两个 Profile、普通备份、过期备份、超预算备份、`pre_restore` 保护点和一个可注入的
partial 删除场景。验收需确认：

- age/space 清理结果与报告一致，保护点保留。
- partial/blocked 在 UI 和 SQLite 重启后仍可解释，重试可收敛。
- 跨 Profile 页面、筛选、备注和恢复入口可用。
- 从备份中心启动恢复仍经过 SAVE-04 预览、确认、恢复前安全备份和完全退出闸门。
- 升级/重启后 Profile、路径、账号展示摘要、retention、历史和备注保留。

## 认证结果（2026-08-16）

SAVE-05 已在 disposable Windows Sandbox 完成受控 synthetic 人工验收。候选代码终点为提交
`7cea779`；验收只使用人工构造的 Alpha/Beta 存档、备份目录和 SQLite，未读取真实 Steam userdata、真实
游戏目录、真实玩家存档或生产 Scheduled Task。

- 数量 retention：Alpha 的普通备份按数量收敛，最新普通备份与 Beta 备份保留，历史事实未被伪删除。
- 空间 retention：16 MiB 上限下物理备份从 3 对收敛到 2 对，释放字节与 UI 摘要一致。
- 保护点与 blocked：恢复前自动备份写入独立 `pre_restore` 保护点；预算无法安全收敛时 UI 显示阻断，
  没有突破保护点或最新普通备份。
- partial/retry：锁定 manifest 时 archive/manifest 出现 `3/4` 的部分清理，备份中心显示需处理项；
  释放锁后显式重试收敛为 `3/3`，需处理归零。
- 年龄 retention：受控 helper 通过正式 writer/repository 写入一份三天前的 Alpha synthetic 备份；
  将 Alpha 策略设为保留数量 `999`、保留天数 `1`、关闭空间上限并执行“立即整理”后，物理状态为
  `archive/manifest=3/3`、`pre_restore=1/1`、已知空间 `23072683` bytes。当前 Alpha `R6`、Beta
  `R0` 和恢复前保护点均保留，旧龄记录在历史中显示为“已整理/不可恢复”。
- 重启持久化：Alpha 的 retention 设置、活动配置档、存档/备份路径、历史、备注、保护点和“需处理 0”
  在完全退出并重启后保持一致；最终 UI 摘要为 9 条历史、22.0 MiB、1 个保护点、0 条需处理。
- 安全边界：受控 helper 不是产品运行时能力，只用于一次性验收；它只接受 disposable Sandbox 的
  synthetic profile/root，并在 HMM 未运行时执行。所有自动化和人工证据均未越过 fixture 映射边界。

残余风险限定为非对抗性本地备份根上的文件系统 TOCTOU：受控句柄复验与按名删除之间仍存在理论上的
并发替换窗口。当前威胁模型不把用户可写备份根视为对抗性并发主体，已有顺序替换、link/junction/reparse
和外部 sentinel 负测；若未来扩大威胁模型，应单独设计平台专属 handle-delete 协议。
