# 存档备份系统设计

本文档定义 T8 存档备份系统的目录策略、命名规范、备份 manifest、后端边界、前端工作流和分阶段落地计划。它是后续实现手动备份、自动备份和恢复能力的设计入口。

关联任务：`TODO.md` T8。

## 目标

- 在不触碰真实游戏目录安装链路的前提下，为玩家存档提供可审计、可校验、可清理的备份能力。
- 未手动选择备份目录时，仍允许使用安全默认目录备份，并在 UI 中明确展示当前备份位置。
- 每个 game/profile 有独立备份文件夹，备份文件名稳定、可排序、可读，不使用随意随机名。
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

## 未来恢复设计边界

恢复能力必须单独切片，并满足：

- 用户二次确认。
- 恢复前读取 manifest 并校验 archive hash。
- 解包到临时目录后校验每个文件 hash/size。
- 恢复前为当前存档目录创建 pre-restore 备份。
- 恢复写入使用同一 profile 的存档写锁。
- 失败时尽量回滚到 pre-restore 备份。
- 写 Audit Log。

没有这些基础前，不允许启用“恢复”按钮。

## 自动备份设计边界

自动备份依赖手动备份服务复用同一条执行链路，只改变 trigger 和触发时机。

后续自动调度要求：

- 启动应用时检查 active profile 的 schedule。
- 只在游戏未运行或用户确认后执行，避免和游戏写存档冲突。
- 同一 profile 的备份任务串行。
- 不因调度失败阻塞应用启动。
- 自动备份失败只展示轻量告警，不循环重试打扰用户。

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

### 切片 4：自动备份

- 复用手动备份服务。
- 接入 schedule。
- 增加应用启动或后台 tick 的调度器。

### 切片 5：恢复能力

- 增加 `preview_restore_save_backup`。
- 增加 `start_restore_save_backup_task`。
- 恢复前二次确认和 pre-restore 备份。
- 恢复完成后刷新历史和审计。

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
