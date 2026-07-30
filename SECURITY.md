# 安全策略

Helsincy Mod Manager 会处理第三方 Mod 压缩包、玩家本地游戏目录和存档备份，因此安全问题会被优先对待。

## 支持范围

当前项目仍处于早期设计和脚手架阶段。随着功能实现，以下问题会被视为高优先级：

- Mod 压缩包路径穿越导致写入沙盒外文件。
- 绝对路径、符号链接或大小写冲突绕过校验。
- 解压炸弹导致磁盘或内存异常占用。
- 伪装图片、损坏图片或超大图片导致崩溃。
- 安装、卸载、回滚逻辑误删玩家文件。
- 存档备份或恢复覆盖错误位置。
- 缺失前置检查导致错误安装提示。
- 日志泄露本地用户名、游戏路径、Steam ID 或其他敏感路径信息。
- Tauri command 暴露危险文件操作能力。
- 自动更新、外部工具执行或插件机制引入远程执行风险。

## 报告方式

请不要在公开 Issue 中直接披露可复现的敏感漏洞细节。

优先使用：

- GitHub 仓库的 Security Advisory 功能。
- 与维护者已经建立的私下沟通渠道。

如果暂时只能通过公开 Issue 联系，请只描述影响范围，不要附带：

- 可直接复现的攻击脚本
- 真实游戏目录或存档路径截图
- Steam 账号相关信息
- 有效 token、cookie、API key
- 可造成误删或覆盖的完整步骤

## 希望报告中包含的信息

请尽量提供：

1. 影响范围。
2. 复现步骤。
3. 触发条件。
4. 预期结果与实际结果。
5. 是否需要特定平台、文件系统或游戏安装方式。
6. 是否涉及真实游戏目录、真实存档或第三方 Mod 包。
7. 脱敏后的日志片段或截图。

## 敏感信息处理约定

提交日志、截图、配置文件或复现材料前，请先脱敏：

- Windows 用户名。
- Steam ID。
- 游戏安装路径。
- 存档路径。
- GitHub token。
- API key。
- Cookie。
- 任何可直接复用的账号凭据。

## 日志和诊断包

日志系统必须遵守 [日志与审计设计](docs/LOGGING.md)。

基本要求：

- 不记录完整本地路径、Windows 用户名、Linux 用户名、Steam ID、token、cookie、真实存档内容或第三方 Mod 内容。
- 高风险文件操作必须进入 Audit Log，但只记录脱敏路径、内部 ID、hash、大小、结果和错误分类。
- 诊断包必须由用户主动导出，并在导出前经过统一脱敏。
- 诊断包不得包含真实存档、第三方 Mod 包、数据库中可还原玩家隐私的信息或未脱敏日志。

## 仓库内安全约束

- 不提交真实 token、密码、cookie、API key。
- 不把真实玩家存档样本提交到仓库。
- 不把第三方 Mod 包直接提交到仓库。
- 测试数据必须使用人工构造的最小样本。
- 新增日志时避免输出完整敏感路径，必要时只输出路径尾部或 hash。
- 新增 Tauri command 时必须确认调用边界和参数校验。
- 新增文件写入逻辑时必须说明备份、回滚和失败恢复策略。

## CLI 自动化边界

- Production CLI 禁止 `--data-dir`。当前只开放 runtime status、game
  status/scan/validate/prerequisites、install plan/status/recovery scan/preview、
  backup list/background status 与 diagnostics snapshot 读取；install apply/uninstall/reinstall/
  recovery apply、backup create/restore/background enable|disable 和 diagnostics export 继续在
  parser 边界不可达。
- Sandbox game 命令只读取显式数据根下的 `config` 和 `fixtures`；保存游戏目录、Steam library 与
  discovery candidate 必须通过词法和 canonical containment。
- `libraryfolders.vdf` 中声明的隔离根外 library 必须在读取 app manifest 前拒绝。
- prerequisite rule path 必须是无 `.`/`..`、盘符或绝对前缀的安全相对路径；只读查询不得 seed
  默认规则、创建目录或 lock/temp 文件。
- Sandbox install 查询只读取显式 data root 下固定的 config、Mod catalog/sandbox、
  manifest/recovery/backup 子路径；已存在的 symlink/junction 不能把读取逃逸到 data root 外。
- Mod catalog 只读模式不创建 lock、不回写 v1 -> v2 migration，并拒绝所有 mutator。install plan
  只输出经 `InstallTargetPath` 校验的逻辑相对 target 与聚合计数，不输出 package/source path。
- profile/mod ID 必须是短的安全标识符，拒绝 `/`、`\`、盘符、`.`/`..` 和其他路径型输入。
- recovery 全量扫描从 manifest/recovery 投影出的 ID 也必须重新校验；篡改后的路径型 ID 和含控制字符
  的逻辑 target 必须在进入 CLI 输出前 fail closed。
- CLI machine projection 不返回完整本地路径、Steam ID、用户名、原始 loader config、自由文本错误
  prerequisite issue path、package file id、backup/recovery ref 或 manifest/sandbox path。
- CLI backup 只读取已 checkpoint 且没有 `hmm.db-wal`/`hmm.db-shm` sidecar 的既有
  `hmm.db`，通过 percent-encoded immutable URI、SQLite read-only flags 和 connection-local
  query-only mode 打开。任一 WAL/SHM sidecar 存在时返回脱敏
  `backup_database_unavailable` 并 fail closed；CLI 不 checkpoint、修复、创建或修改 DB/WAL/SHM，
  也不 migration/seed。immutable 读取不提供跨进程快照锁；需要一致结果时必须在桌面端关闭、
  数据库静止后执行。background status 只读取固定 fake registry/clock，不注册、修复、启动 worker、
  获取 lease 或写 Audit。backup projection 不返回 archive/manifest 文件名、save/backup path、
  Steam ID、source label、notes 或 hash。
- Sandbox diagnostics 只读取固定 `logs/app|tasks|audit`，目录必须通过 canonical containment；
  machine projection 只包含 bounded platform summary、分类状态和计数，不返回日志正文、来源文件名、
  Audit fields、完整本机信息或 export path。
- CLI-2B 的 Sandbox write capability 只在未来写命令显式申请时初始化；`runtime status` 和所有 CLI-1
  只读命令不创建 marker。空根创建固定版本 marker，非空根必须已有完全匹配的 marker。
- marker 不是授权秘密。只有字段/构造器私有且不可序列化的进程内 capability 才能签发
  `SandboxWriteAdmission`；Production 没有构造路径。
- capability 保留 no-follow 根句柄、canonical root 与目录身份，并逐项重验 app-data、game、save、
  backup 根。symlink、junction、reparse point、marker 篡改、祖先替换或 Sandbox 外根全部 fail
  closed。该 capability 不替代 InstallPlan、backup、manifest、rollback/recovery、Audit Log 或写锁。
- 自动测试只使用 temp/fake/人工 fixture，不执行 Production game 命令或读取测试机真实 Steam、
  AppData、游戏、日志和存档，也不查询、注册、更新、启动或删除真实 Windows Scheduled Task。

## Mod 文件安全基线

导入 Mod 包时至少需要考虑：

- 拒绝路径穿越。
- 拒绝绝对路径。
- 拒绝不合理的文件数量、总大小和展开后大小。
- 检测大小写不敏感平台上的路径碰撞。
- 图片必须通过 magic bytes 和解码校验。
- 原始导入包只读保存，安装变体在 staging 目录生成。

## 存档安全基线

存档备份和恢复必须满足：

- 默认备份目录不在游戏安装目录内。
- 支持玩家自定义备份目录。
- 备份结果写入 manifest。
- 恢复前二次确认。
- 自动备份间隔和保留策略可配置。
- 测试不得默认使用真实存档目录。

## 响应原则

- 能确认的问题会尽量复现并评估影响等级。
- 修复完成后再公开必要细节。
- 如果问题来自用户本地环境或第三方 Mod 本身，会尽量说明边界和缓解方式。
- 涉及误删、覆盖、存档损坏的报告会优先处理。
