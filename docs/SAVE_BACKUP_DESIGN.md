# 存档备份系统设计

本文档定义 T8 存档备份系统的目录策略、命名规范、备份 manifest、后端边界、前端工作流和分阶段落地计划。它是后续实现手动备份、自动备份和恢复能力的设计入口。

关联任务：`TODO.md` T8。

## 目标

- 在不触碰真实游戏目录安装链路的前提下，为玩家存档提供可审计、可校验、可清理的备份能力。
- 未手动选择备份目录时，仍允许使用安全默认目录备份，并在 UI 中明确展示当前备份位置。
- 每个 game/profile 有独立备份文件夹，备份文件名稳定、可排序、可读，不使用随意随机名；恢复前安全备份
  必须放在该 profile 下单独的 `pre-restore/` 子目录，不能与普通手动/自动备份混放。
- 备份结果必须写入 manifest 和历史记录，后续恢复只能基于这些事实重新校验。
- 后端负责目录解析、复制/打包、校验、保留策略和审计；前端只展示状态并提交短 id / 用户确认。

## 非目标

首个 T8 切片不实现：

- 自动调度后台备份。
- 备份恢复写入。
- 独立 `features/backups/` 页面。
- Steam Cloud 账号识别或多 Steam 用户自动匹配。
- 从游戏安装目录推断存档路径。

这些能力在手动备份链路和历史事实稳定后再分切片接入。其中存档目录自动发现、多 Steam 用户候选确认和 Steam 资料展示增强由 [存档目录自动发现设计](SAVE_DIRECTORY_AUTO_DISCOVERY_DESIGN.md) 单独规划。

## 现有基础

Profile 模块已经提供：

- `ProfileSaveSettings`：包含 `save_directory`、`backup_directory`、`schedule`、`retention`。
- `ProfileSaveSettingsRepository`：SQLite 持久化 profile 存档设置。
- `ProfileSaveDirectoryValidator`：提供存档目录、备份目录和默认备份目录的选择/展示基础。
- Profile 页面：已有“立即归档当前存档”按钮、备份策略控件和备份历史预览，但按钮和历史仍未接真实后端。

T8 应复用这些设置，不另建一套用户配置。

## 目录策略

### 决策

默认策略采用应用数据目录下的专用备份根，而不是软件安装目录或盘符根目录。

```text
<AppData>/HelsincyModManager/backups/saves/<gameId>/profile-<profileId>/
```

用户手动选择备份根目录时，也不把备份文件直接丢在用户选择的根目录下，而是在根目录内创建受控子目录：

```text
<UserChosenBackupRoot>/HelsincyModManager/saves/<gameId>/profile-<profileId>/
```

未来如需便携模式，可增加显式开关，让默认备份根跟随软件所在盘符：

```text
D:\HelsincyModManager\SaveBackups\<gameId>\profile-<profileId>\
```

便携模式不作为默认行为。理由：

- 软件安装目录可能位于 `Program Files`、用户目录、临时更新目录或便携目录，权限和稳定性不可假设。
- 盘符根目录写入在企业环境、非管理员环境或受控设备上更容易失败。
- 应用数据目录更符合当前安全文档：默认备份目录不在游戏目录内，也不侵入用户任意磁盘根目录。

### profile 文件夹

每个 profile 必须有独立文件夹。文件夹名不使用 profile 显示名，避免重命名导致历史迁移，也避免文件系统非法字符：

```text
profile-default
profile-550e8400-e29b-41d4-a716-446655440000
```

若未来存在不符合安全字符集的 profile id，后端应使用稳定编码或稳定 hash 派生目录名，并在历史记录中保存原始 profile id。

### UI 提示

未手动选择备份目录不是错误。Profile 页面应展示：

- 当前使用“默认备份目录”。
- 默认目录的安全标签，例如 `HelsincyModManager/backups/saves/mhw/profile-default`。
- “更改位置”入口。
- “打开文件夹”入口：默认目录直接定位到该配置档的备份子目录（目录名含内部 id，玩家难以自行辨识，
  由应用带到位；尚未备份过时按需补建托管子树后打开），自定义目录则按玩家所选原样打开。

只有以下情况才阻断备份，并通过页面内状态和居中偏上的悬浮 UI 提示：

- 未配置或未验证存档源目录。
- 存档源目录不可读。
- 备份根目录不可创建或不可写。
- 备份目录与存档源目录存在包含关系风险。
- 本次备份超过安全上限。

## 文件命名规范

备份文件名由后端生成，前端不能提交文件名。

基础格式：

```text
<YYYYMMDD-HHMMSS>_<gameId>_profile-<profileId>_<trigger>.zip
<YYYYMMDD-HHMMSS>_<gameId>_profile-<profileId>_<trigger>.manifest.json
```

示例：

```text
20260704-221530_mhw_profile-default_manual.zip
20260704-221530_mhw_profile-default_manual.manifest.json
```

字段规则：

- 时间使用 UTC 派生的稳定格式，便于排序和跨平台校验；manifest 内保存 UTC 时间戳和明确标注 UTC 的展示标签。后续如果引入可靠本地时区转换，可新增本地展示字段而不是复用 UTC 字段。
- `gameId` 使用稳定短 id，例如 `mhw`。
- `profileId` 使用安全文件名片段，不使用 profile 显示名。
- `trigger` 初始支持 `manual`，后续扩展 `auto`、`pre_install`。
- 同一秒内发生重名时，追加序号：`_02`、`_03`，不使用随机文件名。

用户备注不进入文件名。备注应保存在 manifest 或 SQLite 历史记录中，避免特殊字符、路径长度和隐私问题。

## 备份格式与 manifest

首个实现使用 zip 作为备份包格式，并写同名 manifest sidecar。manifest 是恢复和历史展示的事实来源之一，不能只依赖文件名。

```json
{
  "schemaVersion": 1,
  "backupId": "mhw:profile-default:20260704-221530:manual",
  "gameId": "mhw",
  "profileId": "default",
  "trigger": "manual",
  "createdAtUtc": "2026-07-04T14:15:30Z",
  "createdAtUtcLabel": "2026-07-04 14:15:30 UTC",
  "archiveFileName": "20260704-221530_mhw_profile-default_manual.zip",
  "archiveSizeBytes": 3981200,
  "archiveSha256": "sha256:...",
  "source": {
    "mode": "custom",
    "pathLabel": "582010/remote",
    "pathHash": "sha256:..."
  },
  "files": [
    {
      "relativePath": "SAVEDATA1000",
      "sizeBytes": 3670016,
      "sha256": "sha256:...",
      "modifiedAtUtc": "2026-07-04T13:58:00Z"
    }
  ],
  "notes": null
}
```

manifest 不记录完整本地路径、Steam ID、Windows 用户名、真实存档内容或 zip 内文件内容。`relativePath` 只允许是存档源目录内的相对路径，且必须拒绝父级穿越、绝对路径、空段、symlink/junction 逃逸和大小写碰撞。

## 数据模型

SQLite 新增备份历史表，保存用于 UI 查询和保留策略的摘要。

建议字段：

```text
save_backups
  backup_id TEXT PRIMARY KEY
  game_id TEXT NOT NULL
  profile_id TEXT NOT NULL
  trigger TEXT NOT NULL
  archive_file_name TEXT NOT NULL
  manifest_file_name TEXT NOT NULL
  archive_size_bytes INTEGER NOT NULL
  archive_sha256 TEXT NOT NULL
  file_count INTEGER NOT NULL
  created_at INTEGER NOT NULL
  source_path_label TEXT
  source_path_hash TEXT NOT NULL
  backup_directory_mode TEXT NOT NULL
  backup_directory TEXT
  status TEXT NOT NULL
  notes TEXT
```

`status` 初始支持：

- `completed`
- `deleted_by_retention`
- `missing`
- `invalid`

历史表保存一份最小备份目录快照：`backup_directory_mode` 和可选 `backup_directory`。这只供后端保留策略、后续恢复校验和缺失检测定位旧备份文件使用，不进入前端 DTO、任务事件或日志。用户更换备份根目录后，旧备份仍应按创建时的目录快照清理或校验。

## 后端边界

建议新增 save backup 专用模块，避免把存档备份塞进 Profile 服务或 InstallPlan 服务。

模块职责：

- `hmm-core`
  - 定义 `SaveBackupManifest`、`SaveBackupTrigger`、`SaveBackupStatus`、`SaveBackupRetentionPolicy` 等纯领域模型。
- `hmm-ports`
  - 定义 `SaveBackupRepository`、`SaveBackupFileSystem`、`SaveBackupArchiveWriter`、`SaveBackupAuditLog` 等 trait。
- `hmm-app`
  - `SaveBackupService`：读取 profile 设置、校验源/目标、生成备份计划、执行打包、写 manifest、写历史、应用保留策略。
  - `SaveBackupTaskRunner`：创建 task、发送阶段事件、处理取消和错误。
- `hmm-infra`
  - 文件系统实现：受控读取存档源目录，受控写入备份根目录。
  - SQLite 历史仓储。
  - zip 写入器。
- `hmm-tauri`
  - 窄 command 和 DTO 映射。
- `src/features/profiles` / 后续 `src/features/backups`
  - 展示状态、触发手动备份、刷新历史。

前端和 Tauri command 不得传入最终备份文件名、manifest 文件名、备份目标完整路径或文件列表。

## Tauri command 与 DTO

首个切片建议新增：

```text
start_save_backup_task({ gameId, profileId, note? })
list_save_backups({ gameId, profileId, limit? })
```

`start_save_backup_task` 返回标准 `TaskStartedDto`：

```ts
{
  taskId: string;
  kind: "save_backup";
  status: "queued";
}
```

`list_save_backups` 返回摘要：

```ts
type SaveBackupSummaryDto = {
  backupId: string;
  gameId: string;
  profileId: string;
  trigger: "manual" | "auto" | "pre_install";
  status: "completed" | "deleted_by_retention" | "missing" | "invalid";
  fileName: string;
  createdAt: number;
  sizeBytes: number;
  fileCount: number;
  sourcePathLabel: string | null;
  notes: string | null;
};
```

DTO 不返回完整本地路径、备份根目录、存档源目录、Steam ID、manifest 正文或文件 hash 列表。hash 和完整 manifest 只供后端恢复/校验使用。

## 任务阶段

新增 `TaskKind::SaveBackup`，阶段命名：

```text
save_backup.queued
save_backup.scanning
save_backup.archiving
save_backup.manifest_writing
save_backup.retention_pruning
save_backup.completed
save_backup.failed
save_backup.cancelled
```

阶段规则：

- 所有事件必须携带 `taskId`。
- `scanning` 可统计文件数和总字节数，不写备份。
- `archiving` 写入临时文件，完成后原子移动到目标文件名。
- `manifest_writing` 写同名 manifest sidecar，并写 SQLite 历史。
- `retention_pruning` 只删除本工具生成且历史记录匹配的旧备份文件和 manifest。
- 取消只在扫描和归档阶段协作式生效；如果进入 manifest 写入或保留策略清理，应完成一致性收尾或返回可重试错误。

## 安全校验

备份执行前必须重新校验：

- profile 存在。
- `save_directory` 为 `Valid` 且包含真实目录。
- 备份根目录可解析、可创建、可写。
- 备份目标目录不在存档源目录内部。
- 存档源目录不在备份目标目录内部。
- 存档源目录遍历不跟随 symlink/junction 逃逸。
- 相对路径不包含父级穿越、绝对路径、空段或大小写碰撞。
- 文件数量、单文件大小和总大小不超过策略上限。

首个实现建议使用保守上限：

```text
max_file_count: 200
max_single_file_bytes: 128 MiB
max_total_bytes: 512 MiB
```

这些上限应在后续改为数据驱动设置，但不能写死在前端。

## 保留策略

首个切片至少实现 `maxCount`：

- 只针对同一 `gameId/profileId` 的 completed 备份。
- 按 `createdAt` 从新到旧保留最新 N 个。
- 删除旧 zip 和同名 manifest 时使用该备份历史记录中的备份目录快照，而不是当前 profile 设置。
- 删除文件成功后再将历史状态更新为 `deleted_by_retention`；单个旧备份删除或状态更新失败时继续尝试后续过期备份。
- 保留策略清理失败不应反向判定本次新备份失败；任务应完成，并通过 warning/audit 记录 `save_backup_retention_failed`。

`maxAgeDays` 可与首个切片一起实现，也可以第二切片实现。空间上限需要额外设计，不作为首个切片目标。

## 前端工作流

首个 UI 接入放在 Profile 页面现有区域：

- “立即归档当前存档”按钮在存档源目录未配置/无效时禁用。
- 未配置备份目录时不禁用，因为默认目录可用。
- 点击后调用 `start_save_backup_task({ gameId, profileId })`。
- 按 `taskId` 监听 `save_backup.*` 事件，展示排队、扫描、归档、写 manifest、清理旧备份、完成/失败。
- 完成后刷新 `list_save_backups`。
- 失败时同时在备份卡片内展示错误，并出现居中偏上的悬浮 UI 提示。

历史表首个切片展示：

- 文件名。
- 归档时间。
- 大小。
- 文件数量。
- 触发方式。
- 状态。

备份历史必须使用不依赖横向滚动的响应式布局。每个历史点的“恢复存档”入口属于核心操作，必须在
默认视口始终可见；摘要字段可以换行或在窄宽度下改为上下布局，不能把恢复入口放到需要横向滚动才
能发现的表格右侧。在恢复安全链路尚未完成时，入口保持禁用并明确标注“即将开放”，不能伪装成已
可执行能力。

“恢复”按钮保持禁用，并显示“恢复将在后续切片启用”。恢复不应在没有二次确认、manifest 校验和写入安全设计前开放。

## 错误码

建议稳定错误码：

```text
save_backup_profile_missing
save_backup_source_unset
save_backup_source_invalid
save_backup_source_unreadable
save_backup_clock_unavailable
save_backup_destination_unavailable
save_backup_destination_contains_source
save_backup_source_contains_destination
save_backup_path_escape
save_backup_case_collision
save_backup_size_limit_exceeded
save_backup_archive_write_failed
save_backup_manifest_write_failed
save_backup_history_unavailable
save_backup_retention_failed
```

错误 message 不包含完整路径。UI 根据错误码给出用户可读提示。

## 审计日志

以下操作写 Audit Log：

- 手动备份开始。
- 备份成功。
- 备份失败。
- 保留策略删除旧备份。
- 未来恢复开始、成功、失败。
- 自动备份设置变更。

审计字段只包含：

- `task_id`
- `game_id`
- `profile_id`
- `backup_id`
- `trigger`
- `file_count`
- `archive_size_bytes`
- `result`
- `error_code`

不得记录完整存档目录、备份目录、Steam ID、Windows 用户名、存档内容或 manifest 正文。

## 玩家存档恢复（SAVE-04）

当前状态：代码、temp/artificial fixture 自动化、完整验证、findings-first review 和 disposable Windows
桌面工作流验收均已完成，状态为 `certified`。验收证据见 [SAVE-04 验收记录](SAVE_04_ACCEPTANCE.md)。

恢复能力必须单独切片，并满足：

- 用户二次确认。
- 恢复前读取 manifest 并校验 archive hash。
- 解包到临时目录后校验每个文件 hash/size。
- 恢复前为当前存档目录创建 pre-restore 备份；默认开启，且必须先成功落盘再允许覆盖。
- 恢复写入使用同一 profile 的存档写锁。
- 失败时尽量回滚到 pre-restore 备份。
- 写 Audit Log。

恢复 UX 与安全默认：

- 点击 Profile 中的“恢复存档”后，必须打开统一的悬浮/Modal 确认层，明确显示来源备份摘要、目标
  Profile、恢复前自动备份开关和不可逆风险；不能使用卡片内联展开替代确认层。
- `pre_restore_backup_enabled` 持久化在现有 Profile 存档设置中，默认值为 `true`。用户关闭后，
  确认层必须显示高风险警告并要求再次确认；关闭不应被静默继承到其他 Profile。
- 预恢复备份目录为：
  `<backup-root>/HelsincyModManager/saves/<gameId>/profile-<profileId>/pre-restore/`。
  文件名使用 `<UTC>_<gameId>_profile-<profileId>_pre-restore.zip` 及同名 manifest，允许序号后缀，
  不使用 Profile 显示名、Steam ID 或完整本地路径。
- 备份、恢复和最终结果都要通过 task progress、持久成功/失败通知与 Audit Log 呈现；备份失败时
  fail closed，不开始恢复。

Profile 备份历史只对状态为 `completed` 的记录启用恢复入口；其他状态继续 fail closed。

### SAVE-04 恢复事实与事务契约

`backupId` 是备份 writer 生成并持久化的 opaque identity，不是文件名、路径或可由前端解释的复合键。
当前 canonical writer 格式为
`<gameId>:<profileId>:<UTC timestamp>:<trigger>[:<sequence>]`（4 或 5 个 ASCII 分段）；同一时间槽
发生文件名冲突时才出现第五段序号。各分段只允许字母、数字、`-`、`_`、`.`，不得为空。为兼容
早期持久化数据，边界可接受受控的单段 legacy ID，但不接受任意冒号拼接、盘符、斜杠或路径形状。
前端必须原样保存和回传该值，不得拆分、重建、规范化或将其当作路径。

恢复以已持久化的 `(gameId, profileId, backupId)` 为唯一来源。前端只能提交这三个短 id、后端签发的
短时 preview token，以及受控确认位；不得提交 archive/manifest/目标目录、文件名、hash 列表或任意
路径。后端按历史记录中的备份目录快照定位 archive 和 manifest，并按以下顺序生成预览：

1. 精确读取 backup summary，要求 game/profile/backup identity 一致且状态为 `completed`。
2. 读取 manifest，校验 schema、identity、trigger、archive 文件名、size 和 SHA-256 与 summary 一致。
3. 对 archive 逐 entry 流式读取与校验，preview 不创建恢复 staging；拒绝绝对路径、盘符/UNC、`..`、
   空段、目录 entry、symlink/reparse、重复 entry、大小写碰撞、未知 entry，以及文件数/单文件/总大小越界。
4. 按 manifest 逐文件校验 relative path、解压流 size 和 SHA-256；不得把 ZIP 列表直接当作可信事实。
5. 读取当前 Profile settings 和游戏运行状态。目标存档目录必须仍为已验证目录；`Running` 与
   `Unknown` 都阻断恢复。
6. 对无路径的结构化事实生成 digest，签发短时 opaque preview token。预览 DTO 只包含来源时间、
   trigger、文件数、总大小、目标 Profile 摘要、是否启用 pre-restore 和受控 warning/block code。

commit 不信任 preview 时读取的路径或临时解包结果。任务启动后在锁外重新执行完整来源校验，并把
archive 解包为目标同父目录下的受控 staging；manifest/hash/path/size 校验、staging 摘要和当前目标摘要
均在等待共享 `GameProfileWriteLockRegistry` 前完成。获取锁后只重新读取 Profile settings、backup、
游戏运行状态和事务等短事实，复用已验证 source facts 校验 preview token/facts digest，并重新计算目标与
staging 摘要；锁内不再次解压或执行完整 archive/hash 扫描。Mod 安装、卸载、重装、安装恢复和存档恢复
必须由 runtime 注入同一个 registry，使同一 game/profile 的提交串行。

### pre-restore 与保留策略

`ProfileSaveSettings.pre_restore_backup_enabled` 默认 `true`，按 Profile 持久化；migration 012 会把
既有 Profile 显式迁移为开启。开启时，任务在等待共享写锁前通过现有备份 writer 创建
`SaveBackupTrigger::PreRestore` 备份并写 manifest/SQLite；只有 archive、manifest 和历史记录全部成功
后才允许进入锁内复核与覆盖。失败必须 fail closed。锁外安全备份不会放宽提交门禁：如果备份后目标
内容发生变化，锁内目标摘要复核会拒绝提交。

pre-restore 备份写入 profile 根下独立 `pre-restore/`，文件名为
`<UTC>_<gameId>_profile-<profileId>_pre-restore[序号].zip` 及同名 manifest。它不参与普通
manual/auto/pre-install 的 `maxCount` retention，避免一次恢复挤掉用户的常规备份；后续 SAVE-05 再为
该目录定义独立 retention/空间策略。历史 DTO 必须明确显示 trigger 为 `pre_restore`。

关闭 pre-restore 时，preview 显示高风险 warning，提交请求必须同时提供 `confirmed=true` 与
`confirmed_without_pre_restore=true`；后端仍以当前持久设置为准，不能由前端临时关闭安全备份。

### 短提交、回滚与崩溃恢复

恢复 executor 只能操作已验证的目标存档根及其同父目录的应用拥有 sibling。进入覆盖前持久化恢复事务
事实，至少记录 transaction id、game/profile/backup id、pre-restore backup id、阶段和脱敏错误码；
不得记录完整路径或 manifest/hash 列表。

锁内短提交采用目录交换语义：把验证完成的 restore staging 物化到目标同父目录的受控 sibling，重验
目标和 sibling 均非 link/reparse 后，将现有目标改名为 rollback sibling，再将 restore sibling 改名为
目标。交换成功后先持久化非终态 `Committed`，再幂等清理 rollback/failure evidence；只有清理成功后才
持久化 `Completed`。finalize 失败持久化 `RecoveryRequired` 并保留可重试 stage。平台不支持可靠目录交换
或目标无法证明归属时必须 fail closed，不能退化为逐文件覆盖。

若第二次改名或后续一致性检查失败，优先用仍在同父目录的 rollback sibling 恢复原目标；若该事实不可用，
再从已完成的 pre-restore archive 重新校验并恢复。能够确认原状态恢复时任务返回失败但不要求人工恢复；
无法确认时持久化 `recovery_required`，保留事务事实和受控 sibling，任务/通知/Audit 明确提示需要恢复，
不得伪报成功或静默删除证据。runtime 启动时后续可据事务事实扫描未完成恢复；SAVE-04 至少提供状态读取
和显式失败投影，不复用 Mod InstallPlan recovery manifest。

prepare 会在进程内保留目标父目录 capability、父/目标/staging identity 和受控子项名称；commit、rollback、
discard 与 finalize 都只能相对该 capability 操作，并在每次目录交换前后复核 identity 与 digest。该 capability
不持久化，也不能在应用崩溃或重启后从绝对路径安全重建。因此提交后、durable `Completed` 前发生进程
终止时，重启后的行为必须保留 `Committing`/`Committed` 非终态事务和仍存活的磁盘 sibling、阻断新的恢复
并 fail closed。finalize 本身幂等记录逐 child 清理进度；若进程在清理中终止，部分 sibling 可能已安全删除，
其余 evidence 仍保留。当前切片不承诺自动跨进程收尾，人工验收和支持流程必须把这类现场视为 recovery
evidence，而不是可直接删除的临时文件。

### SAVE-04 command、task 与错误码

新增窄 command：

```text
preview_save_restore({ request: { gameId, profileId, backupId } })
start_save_restore_task({ request: {
  gameId,
  profileId,
  backupId,
  previewToken,
  confirmed,
  confirmedWithoutPreRestore
} })
```

恢复使用独立 `TaskKind::SaveRestore` 和阶段：

```text
save_restore.queued
save_restore.preparing
save_restore.pre_restore_backup
save_restore.revalidating
save_restore.committing
save_restore.completed
save_restore.failed
save_restore.recovery_required
save_restore.cancelled
```

`preparing`、pre-restore 备份、等待锁和 `revalidating` 完成前可取消；持久化 `Committing` 前启用
cancellation barrier，之后必须先完成提交或回滚收尾。取消 safe point 必须先把事务持久化为
`Failed + save_restore_cancelled`，成功后才能清理 prepared staging 并发送 cancelled 终态；若该写入失败，
保留 staging/未完成事务并发送 `recovery_required + save_restore_transaction_unavailable`。该后端终态允许
覆盖 transport 或 command response 先投影的乐观 cancelled 状态。稳定错误码区分 profile/backup/target、
archive/manifest/hash/path/size、游戏运行状态、token、确认、pre-restore、scope/lock、facts drift、
commit/rollback/recovery 和 evidence degradation。事务 durable `Completed` 后，TaskManager completion 或
success Audit 写入失败只在 completed event 投影 `save_restore_evidence_degraded`，不得伪造业务失败、
failure Audit 或玩家文件回滚。`failed` 事件的 `error` 是主错误码；若已回滚但 finalize 失败，`message` 可携带
受控二级 warning code，前端必须同时展示“已回滚”和 evidence 清理警告。错误 message、Task Log、Audit 和
通知不得包含本地路径、Windows 用户名、
Steam ID、manifest 正文、hash 或存档内容。

恢复登记在 `SaveRestoreTaskScopeRegistry` 中保留 game/profile scope，直到 runner 发出 terminal 并完成 scope
release。窗口完全退出在后端通过同一 registry 原子关闭新的 restore admission：已有 queued/running restore
或 scope 无法读取时一律返回稳定 `blocked` 原因，不生成 override authorization，也不能借后台保护危险退出
绕过。此时 UI 只可返回应用或收起至托盘，防止 event loop 在目录交换、rollback 或 finalize evidence 收尾期间
终止 runner。

## 自动备份设计边界

自动备份依赖手动备份服务复用同一条执行链路，只改变 trigger 和触发时机。

自动备份不能只依赖前端定时器或主窗口打开期间的 Tauri 进程。正式宣称“自动备份可用”前，必须满足 [自动备份后台保障设计](SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md) 中定义的关闭窗口、退出主客户端和系统恢复后的语义。

后续自动调度要求：

- 主窗口关闭后默认进入托盘常驻，客户端内调度仍可继续执行。
- 用户真正退出主客户端后，若已启用后台保障，应由用户级后台守护或系统计划任务接管。
- 若后台保障未启用或不可用，UI 必须明确提示自动备份只在客户端运行期间生效。
- 启动应用或后台守护恢复时检查 active profile 的 schedule，并对错过的计划执行一次追赶备份。
- 只在游戏未运行时无人值守执行，避免和游戏写存档冲突；游戏运行中应记录 pending 并延后。
- 同一 profile 的备份任务串行。
- 不因调度失败阻塞应用启动或后台守护启动。
- 自动备份失败必须写入历史/审计；主界面打开时展示轻量告警，不循环重试打扰用户。

## 分阶段落地

### 切片 1：设计文档

- 新增本文档。
- 明确目录策略、文件名规范、manifest schema、DTO、任务阶段、安全校验和测试门禁。

### 切片 2：手动备份后端 MVP

- 新增 core/ports/app/infra/Tauri 最小链路。
- 新增 `start_save_backup_task`、`list_save_backups`。
- 写 zip、manifest、SQLite 历史。
- 实现 `maxCount` 保留策略。
- 写单元测试和临时目录集成测试。

### 切片 3：Profile 页面接入

- 启用“立即归档当前存档”按钮。
- 接入任务进度和历史刷新。
- 加入失败悬浮 UI。
- 保持恢复按钮禁用。

### 切片 4：客户端内自动备份

- 复用手动备份服务。
- 接入 schedule。
- 增加应用启动或后台 tick 的调度器。
- 主窗口关闭后进入托盘常驻时继续检查计划。
- 明确展示“仅客户端运行时受保护”的状态。

### 切片 4b：自动备份后台保障

- 按 [自动备份后台保障设计](SAVE_BACKUP_BACKGROUND_AUTOMATION_DESIGN.md) 新增用户级后台守护或系统计划任务。
- 调度状态、租约去重和 worker 健康内核按 [后台自动备份调度内核实现计划](SAVE_BACKUP_BACKGROUND_SCHEDULER_CORE_PLAN.md) 分切片落地。
- 新增调度状态、worker 健康检查和后台保障状态展示。
- 支持主客户端退出后的计划检查、overdue 追赶和 pending 备份。
- 覆盖后台守护与主客户端调度器并发时的去重。

### 切片 5：恢复能力

- [x] 增加 `preview_save_restore`。
- [x] 增加 `start_save_restore_task` 与独立 `TaskKind::SaveRestore`。
- [x] 统一悬浮确认层，默认开启恢复前 pre-restore 备份；关闭开关时显示风险并要求额外确认。
- [x] pre-restore 备份写入独立目录，成功后才允许恢复；失败时 fail closed 并保留可审计结果。
- [x] 恢复完成后刷新历史，并保留 Task/Audit/evidence degradation 投影。
- [x] disposable Windows 人工验收与最终 `certified` 门禁。

### 切片 6：独立备份中心页面

- 新增 `features/backups/`。
- 支持跨 profile 浏览、筛选、备注和恢复入口。

## 测试要求

首个实现 PR 至少覆盖：

- 默认备份目录解析到 app data 下的受控路径。
- 自选备份根目录下仍创建规范子目录。
- 每个 profile 的备份进入独立文件夹。
- 文件名符合规范，同秒碰撞追加序号。
- 写 zip 和同名 manifest。
- manifest 不含完整本地路径或 Steam ID。
- 源目录 symlink/junction 逃逸被拒绝。
- 源/目标包含关系被拒绝。
- 大小写路径碰撞被拒绝。
- 备份目录不可写时返回稳定错误码。
- `maxCount` 保留策略只删除同 profile 的旧备份。
- `pre_restore` 备份写入独立目录，且不被普通 `maxCount` retention 删除。
- restore preview/token 对 backup、manifest、archive、逐文件 hash/size/path 和目标状态 fail closed；
  token stale/expired、目标 drift、游戏运行和未知状态均阻断提交。
- 默认开启与关闭 pre-restore 的二次确认、pre-restore 失败不写目标、同 profile 写锁串行、commit barrier
  取消、`Committing -> Committed -> Completed`、目录交换 rollback、幂等 finalize 重试和
  recovery-required evidence 均有正负 fixture。
- durable `Completed` 后 Task/Audit 写入失败只投影 evidence degradation，不伪造业务失败或回滚玩家文件。
- 任务事件携带 `taskId`。
- Audit Log 不含完整路径。

所有测试必须使用临时目录或 fake file system，不依赖真实 MHW:I 安装目录、真实 Steam userdata 或真实玩家存档。

## 文档同步要求

实现 T8 任一切片时，需要按影响范围同步检查：

- `TODO.md`
- `docs/ROADMAP.md`
- `docs/ARCHITECTURE.md`
- `docs/FRONTEND_BACKEND_CONTRACT.md`
- `docs/TESTING.md`
- `docs/LOGGING.md`
- `SECURITY.md`

尤其当新增 command、DTO、task phase、错误码、SQLite schema、Audit Log 字段或恢复行为时，必须同步契约和测试文档。
