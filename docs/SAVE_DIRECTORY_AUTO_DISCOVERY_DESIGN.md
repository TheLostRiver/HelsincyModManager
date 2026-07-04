# 存档目录自动发现设计

本文档定义 MHW:I 存档目录自动发现、Steam 多账户候选确认、公开 Steam 资料补全、Profile 存档设置写入和前端提示策略。它是 `docs/SAVE_BACKUP_DESIGN.md` 的后续切片，不改变手动备份执行链路、备份文件命名、manifest 或恢复策略。

关联任务：`TODO.md` T8。

实现计划：[`docs/superpowers/plans/2026-07-05-save-directory-auto-discovery-implementation.md`](superpowers/plans/2026-07-05-save-directory-auto-discovery-implementation.md)。

## 背景

当前项目已经具备两块基础：

- 游戏扫描成功后会持久化 `GameInstance`，后续启动和安装恢复检查可以复用已保存的游戏路径。
- Profile 存档设置已经有 `ProfileSaveSettings.save_directory`，手动备份只在存档源目录有效时才允许执行。

MHW:I 的 Steam 存档通常不在游戏安装目录下，而是在 Steam userdata 下：

```text
<SteamRoot>/userdata/<account_id_32>/582010/remote/
```

因此“根据游戏路径自动选择存档路径”不能由前端拼接，也不能只看游戏安装目录。正确方向是后端基于 Steam root、MHW:I app id、userdata 规则和存档目录验证来发现候选，再在安全条件满足时写入 Profile 设置。

## 目标

- 游戏扫描成功或应用启动自检时，自动发现当前游戏的可用存档目录。
- 只有一个高置信候选时，自动写入当前 active profile 的 `save_directory`。
- 多个 Steam 用户候选时，默认推荐最近修改的候选，但必须让用户确认。
- 多候选确认 UI 展示可理解的 Steam 资料：昵称、头像、最近修改时间和脱敏路径标签，不只显示数字账号 ID。
- 网络失败、Steam 资料私密或接口返回异常时，发现流程仍可用，只降级展示。
- 所有真实路径、Steam ID、存档内容和账号资料处理都遵守现有安全与日志边界。

## 非目标

- 不执行备份、恢复、安装、卸载、manifest 写入或保留策略清理。
- 不把 Steam Web API key、OAuth、登录态或 cookie 引入项目。
- 不上传、记录或导出真实存档内容。
- 不要求测试环境安装真实 MHW:I、真实 Steam 或真实玩家存档。
- 不让前端根据 `gameId`、游戏名、Steam ID 或路径标签拼接真实文件系统路径。
- 不静默覆盖用户已经手动选择并验证通过的存档目录。

## 关键决策

### 1. 唯一候选自动写入，多候选必须确认

自动发现结果分三类：

| 结果 | 行为 |
| --- | --- |
| 0 个候选 | 不写入设置，显示居中偏上的悬浮提示，并保留手动选择入口。 |
| 1 个高置信候选 | 后端重新验证后写入当前 active profile 的 `save_directory`。 |
| 多个候选 | 按最近修改时间推荐一个，但前端必须展示候选列表并等待用户确认。 |

“高置信候选”至少要求：

- `remote` 目录存在。
- 目录可读。
- 路径位于受支持 Steam root 的 `userdata/<account_id_32>/582010/remote` 下。
- account id 目录名是十进制数字。

若目录内存在 MHW:I 常见存档文件，例如 `SAVEDATA1000`，候选置信度提高并可参与唯一候选自动写入。空目录或缺少常见存档文件的候选可以展示给用户，但不作为静默自动写入的唯一依据。

### 2. Steam 资料只做展示增强

多候选确认时，后端可以把 userdata 的短 account id 转换为 SteamID64，并由 `hmm-infra` 发起公开 profile XML 查询：

```text
steam_id_64 = 76561197960265728 + account_id_32
GET https://steamcommunity.com/profiles/<steam_id_64>/?xml=1
```

只使用以下字段：

- `steamID`：作为用户可读昵称。
- `avatarMedium` 或 `avatarFull`：作为候选头像。
- `steamID64`：仅用于校验响应是否对应请求，不进入前端展示、日志或诊断包。

查询规则：

- 请求必须由后端发起。前端不直接访问 Steam Community，不计算 SteamID64，不解析 XML，避免 CORS 问题和外部接口逻辑泄漏到 UI。
- 只在多候选需要用户确认时默认触发，减少不必要的网络请求。
- 单个请求设置短超时，例如 2 秒。
- 请求失败、资料私密、XML 缺字段或解析失败时，候选仍然可展示和选择。
- 头像 URL 只接受 HTTPS 且来自 Steam 头像域名；不符合预期时丢弃头像。
- 不使用 ad hoc 字符串截取解析 XML，应使用 XML 解析器读取字段。

### 3. 前端只消费候选摘要和 opaque id

前端候选列表只接收后端生成的摘要，不接收完整存档路径、完整 Steam ID 或 account id。头像 URL 也是后端校验后的展示字段，前端只作为普通图片资源消费，不参与 URL 生成或 profile 查询。

建议 DTO：

```ts
type SaveDirectoryDiscoveryDto = {
  discoveryId: string;
  gameId: string;
  profileId: string;
  outcome:
    | "auto_saved"
    | "confirmation_required"
    | "not_found"
    | "existing_valid"
    | "existing_invalid"
    | "scan_failed";
  recommendedCandidateId: string | null;
  candidates: SaveDirectoryCandidateDto[];
  savedSettings?: ProfileSaveSettingsDto;
};

type SaveDirectoryCandidateDto = {
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
```

展示字段示例：

- `accountName`: `玩家昵称`，失败时为空。
- `accountLabel`: `Steam 用户 ****1234`。
- `pathLabel`: `Steam/userdata/<account>/582010/remote`。
- `evidence`: `["找到 MHW:I 存档文件", "最近修改：2 小时前"]`。

`candidateId` 和 `discoveryId` 是后端生成的 opaque id。用户确认时前端只提交：

```ts
confirm_profile_save_directory_candidate({ discoveryId, candidateId })
```

后端根据短期缓存取回真实路径并重新验证。缓存过期时返回稳定错误，前端重新触发发现流程。

### 4. 已有有效设置不被覆盖

启动自检或游戏扫描成功后：

- 如果当前 profile 的 `save_directory` 已有效，只展示“存档目录正常”的轻量状态，不覆盖。
- 如果 `save_directory` 已配置但失效，返回 `existing_invalid`，悬浮提示用户重新检测或手动选择。
- 如果 `save_directory` 未配置，执行自动发现。

用户通过确认候选写入后，该路径成为普通 `ProfileSaveSettings.save_directory`，后续备份仍走现有手动备份校验链路。

## 后端边界

建议新增独立服务，避免把发现逻辑塞进 Profile CRUD 或 SaveBackup 执行服务。

```text
hmm-core
  SaveDirectoryDiscoveryResult
  SaveDirectoryCandidate
  SteamAccountProfileSummary

hmm-ports
  SaveDirectoryDiscoveryService trait
  SteamUserdataScanner trait
  SteamAccountProfileClient trait
  PendingSaveDirectoryCandidateStore trait

hmm-app
  ProfileSaveDirectoryDiscoveryService
    - 读取 GameInstance
    - 读取 active/requested profile
    - 调用 Steam userdata scanner
    - 调用 Steam profile client 补全摘要
    - 应用唯一候选自动写入规则
    - 处理多候选确认写入

hmm-infra
  Steam userdata 文件系统扫描
  Steam profile XML 查询与解析
  短期候选缓存

hmm-games-mhw
  MHW:I 存档目录规则
    - steam app id: 582010
    - userdata remote 目录规则
    - 可选存档文件证据，如 SAVEDATA1000

hmm-tauri
  discover_profile_save_directories
  confirm_profile_save_directory_candidate

src/features/profiles
  Profile 页面存档目录自动检测入口
  多 Steam 用户候选确认 UI
  悬浮提示
```

Tauri command 保持薄边界，只做 DTO 映射和调用 app service。真实路径解析、Steam userdata 扫描、账号资料查询、候选排序和写入设置都在后端完成。

## 数据与缓存

### Profile 设置

最终被用户确认或唯一候选自动写入的真实存档路径继续保存在本地 SQLite 的 `profile_save_settings.save_directory` 中。这是本机配置事实，不进入前端 DTO、任务事件、日志或诊断包。

### 候选缓存

多候选发现结果建议短期保存在后端内存缓存：

- key: `discoveryId`
- value: 候选 id 到真实路径、account id、验证证据的映射
- TTL: 10 分钟

这样前端确认时不需要携带真实路径。若未来需要跨重启恢复候选，再单独设计持久化缓存；首个切片不需要。

### Steam 资料缓存

公开 Steam 资料可做短 TTL 缓存：

- key: account id hash 或 SteamID64 hash。
- value: `accountName`、`avatarUrl`、`fetchedAt`、`status`。
- TTL: 24 小时。

缓存中不保存完整 Steam ID、完整本地路径或存档内容。若实现成本需要，可以首个切片只做进程内缓存，不写 SQLite。

## 前端交互

### 自动成功

唯一候选自动写入时：

- Profile 页存档目录卡片显示 `valid`。
- “立即归档当前存档”按钮可用。
- 居中偏上的悬浮 UI 显示“已自动关联 MHW:I 存档目录”，几秒后自动消失。

### 多账户确认

多候选时：

- 居中偏上的悬浮 UI 提示“发现多个 Steam 存档账户，需要确认”。
- 悬浮提示几秒后自动消失，但 Profile 页保留确认入口。
- 确认入口展示候选卡片：
  - 头像或占位图。
  - Steam 昵称；不可用时显示“Steam 资料不可用”。
  - 脱敏账户标签。
  - 最近修改时间。
  - `推荐` 标记，默认给最近修改的候选。
  - 选择按钮。

确认后：

- 前端调用 `confirm_profile_save_directory_candidate`。
- 后端重新验证并写入 `ProfileSaveSettings`。
- UI 刷新 settings 和备份按钮状态。

### 没找到或失败

没有候选或扫描失败时：

- 居中偏上的悬浮 UI 提示，几秒后自动消失。
- Profile 页存档目录卡片保持 `unset` 或 `invalid`。
- 保留“自动检测”和“手动选择路径”两个入口。

## 安全与隐私

- 不在日志、Audit Log、任务事件、DTO 或诊断包中记录完整本地路径、完整 Steam ID、account id、Windows 用户名或真实存档内容。
- 不把 Steam 资料 XML 原文写入日志或诊断包。
- 不提交真实 Steam XML 样本、真实头像 URL、真实本地路径或真实存档样本到仓库。
- 外部网络请求仅访问公开 Steam profile XML；没有 token、cookie 或 API key。
- 网络请求失败不阻塞用户选择候选，也不阻塞手动选择路径。
- 头像 URL 必须经过协议和域名白名单过滤。
- 备份执行前仍由 `SaveBackupService` 重新校验 `save_directory`，自动发现结果不能绕过备份安全检查。

## 错误码

建议稳定错误码：

```text
save_directory_discovery_game_unconfigured
save_directory_discovery_profile_missing
save_directory_discovery_scan_failed
save_directory_discovery_not_found
save_directory_discovery_multiple_candidates
save_directory_discovery_candidate_expired
save_directory_discovery_candidate_invalid
save_directory_discovery_profile_lookup_failed
save_directory_discovery_settings_unavailable
```

错误 message 不包含完整路径、完整 Steam ID、account id 或 XML 原文。前端基于 code 显示用户文案。

## 测试要求

后端聚焦测试：

- 使用 temp Steam root 构造 `userdata/<account_id_32>/582010/remote`。
- 覆盖 0 候选、1 高置信候选、多候选、空 remote、缺少常见存档文件。
- 覆盖多候选按最近修改时间推荐，但不自动写入。
- 覆盖唯一高置信候选自动写入 `ProfileSaveSettings.save_directory`。
- 覆盖已有有效设置不被覆盖，已有失效设置返回 `existing_invalid`。
- 覆盖 account id 到 SteamID64 的 u64 转换。
- 使用人工 XML fixture 覆盖 `steamID`、`avatarMedium`、`avatarFull` 解析。
- 覆盖 XML 解析失败、HTTP 失败、超时、资料缺字段时降级展示。
- 覆盖 DTO 和错误不包含完整路径、完整 Steam ID 或 account id。
- 覆盖确认候选时缓存过期、候选失效、重新验证失败。

前端聚焦测试：

- 多账户状态必须展示确认入口，不自动提交候选。
- 候选卡片必须有昵称或资料不可用文案、头像/占位图、推荐标记和最近修改时间。
- 不能只用裸 Steam ID 作为候选主要文案。
- 悬浮 UI 出现在居中偏上位置，普通提示几秒后自动消失。
- 确认后刷新 Profile 存档设置和备份按钮状态。

所有测试必须使用 fake/temp 数据，不依赖真实 MHW:I 安装目录、真实 Steam userdata、真实玩家存档或真实网络。

## 分阶段落地

### 切片 1：设计文档

- 新增本文档。
- 在 README、TODO 和存档备份设计中挂引用。

### 切片 2：后端发现与确认

- 增加 MHW:I 存档目录规则。
- 增加 Steam userdata scanner。
- 增加 `discover_profile_save_directories` 和 `confirm_profile_save_directory_candidate`。
- 覆盖 temp fixture 测试和 DTO 脱敏测试。

### 切片 3：Steam 资料补全

- 增加公开 profile XML client。
- 增加 SteamID64 转换和 XML parser。
- 增加短超时、降级和头像 URL 白名单。
- 覆盖 fake HTTP client 测试。

### 切片 4：Profile 页接入

- Profile 存档目录卡片增加“自动检测”入口。
- 多候选确认 UI 展示头像、昵称、推荐候选和最近修改时间。
- 悬浮 UI 接入自动成功、未找到、失败和需要确认状态。

### 切片 5：启动后台自检

- 游戏目录已配置时，在应用启动或 Dashboard 初始化后静默检查 active profile。
- 唯一高置信候选自动写入。
- 多候选/未找到/失效只提示，不阻塞应用启动。
