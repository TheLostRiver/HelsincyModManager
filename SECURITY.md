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

- Production CLI 禁止 `--data-dir`，数据根仅由操作系统解析。只读命令继续开放 runtime status、
  game status/scan/validate/prerequisites、install plan/status/recovery scan/preview、
  backup list/background status 与 diagnostics snapshot。CLI-3B 起 install apply/uninstall/
  reinstall/recovery apply 四条单项 lifecycle 命令按 command 开放 Production 写入：每条命令要求
  production 环境签发的 5 分钟 opaque token、显式 `--commit --yes`、CLI-3A `game-profile-write`
  跨进程 admission，以及 game/profile 写锁内的 token/事实/游戏根一致性重验；production 与
  sandbox token 的环境标签参与 digest，跨环境重放必然失效。install batch 在 automation 边界继续
  开放 Production（CLI-3C：token 由 per-installation secret 签名，纯语法预检在触达数据根前
  拒绝非法/过期 token，锁外 register 记录已保存配置游戏根、锁内重载一致性重验）；
  backup create/restore/background
  enable|disable 和 diagnostics export 继续在 parser 边界不可达。
- Production 写侧没有 sandbox marker；对应的根事实是 prepare 阶段从已保存配置读取的游戏根，
  锁内重载配置并要求与锁外记录一致且仍然存在为目录，配置漂移 fail closed。CLI 不接受任何
  调用方提交的目标路径。
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
- CLI-2B 的 Sandbox write capability 只在 CLI-2C lifecycle 写命令显式申请时初始化；
  `runtime status` 和所有 CLI-1 只读命令不创建 marker。空根创建固定版本 marker，非空根必须已有
  完全匹配的 marker。
- marker 不是授权秘密。只有字段/构造器私有且不可序列化的进程内 capability 才能签发
  `SandboxWriteAdmission`；Production 没有构造路径。
- capability 保留 no-follow 根句柄、canonical root 与目录身份，并逐项重验本次操作提交的 game、
  save、backup 根以及（批量 / CLI 沙箱链）app-data 根。symlink、junction、reparse point、marker 篡改、
  祖先替换或 Sandbox 外根全部 fail closed。该 capability 不替代 InstallPlan、backup、manifest、
  rollback/recovery、Audit Log 或写锁。
- #273 起 GUI 组合只把游戏根放进准入集（`SandboxWriteRoots::game_root_only`）：GUI 的 app-data 由
  Tauri 解析到系统位置、不会迁入沙箱根，要求它被包含会让 GUI 写入结构性不可通过；这是接受的削弱，
  沙箱模式保护的对象是游戏根。批量 / CLI 沙箱链的数据根就是沙箱根本身，app-data containment 语义不变。
- #275 起 Mod 存储根（`sandboxes/<packageId>/` 所在目录，默认 app-data 下的 `mod-import`）可由用户
  配置到任意盘。它与 app-data 同类：GUI 不把它纳入沙箱 containment；CLI 沙箱链要求解析后的
  `sandboxes/` 仍在 `--data-dir` 内。用户目录只在空目录或已带 HMM marker（`.hmm-mod-storage.json`，
  字节精确）的目录上认领，非空外来目录一律拒绝；目录本身与全部祖先不得是 symlink / junction /
  reparse point；与任一已配置游戏根双向互不包含；设置前做 `create_new` + 删除的试写探针。运行时对
  存储根的所有读写仍经 no-follow 句柄链（`open_managed_sandbox_root` → `sandboxes` → `<packageId>`），
  删除只删 `<存储根>/sandboxes/<packageId>`。配置目录启动时不可用（外接盘拔掉、marker 被删）时保持
  指向该目录并降级报告，不回落默认根——否则新导入会散落到另一处。目标目录还不得与当前存储根
  相同、互相包含或落在其 `sandboxes/` 之下（`mod_storage_dir_overlaps_current_root`）。
- 库非空时换目录只能走迁移任务（#275 切片②）：逐包复制到新根并回读比对文件集合 / 大小 / SHA-256，
  全部通过后才写 `settings.json`；复制、校验、删除全程走 no-follow 句柄，包内或 `sandboxes/` 下出现
  symlink / junction / reparse point 即整体失败。任一步失败或取消都删掉目标里本次复制的包、设置不变；
  旧根副本只在**下次启动**新根生效后、逐包确认新根副本存在后才删除，崩溃留下的 journal
  （`<app-data>/mod-import/migration.json`）在下次启动收尾或回滚，源根不可用时保留不动。迁移登记到
  重启之间，导入 / 外部导入 / 删除等沙箱写入一律拒绝（`mod_storage_migration_in_progress` /
  `mod_storage_restart_required`），读路径不受影响；有导入任务在跑时拒绝启动迁移。任务事件与日志
  只带包计数与稳定码，不带路径或包名。
- 「移动导入」（#275 切片④，默认关）是 install executor 之外唯一会删除用户文件的路径，且只删用户自己在
  系统文件选择器里选中的那个源压缩包：导入前取指纹（长度、修改时间、卷 + 文件索引），目录写入 durable
  后 no-follow 重开、比对指纹一致才删；目录 / 链接 / reparse point、导入期间被替换、位于任一游戏根 /
  当前存储根 / app-data 之内、游戏配置读不到，全部保留不删并以 `mod_import_archive_kept_*` 降级码上报。
  外部导入（HuntingBox 目录）的压缩包永不删除。
- lifecycle preview 只在 ready/available 时签发 5 分钟 opaque token；Sandbox 与 Production
  各自签发（CLI-3B），环境标签参与 digest，跨环境重放一律 `plan_token_invalid`。提交同时要求
  `--commit --yes`；token 绑定 command、环境、受控 ID 和计划/manifest/recovery
  结构化状态摘要，不包含路径，并在装配写 runtime 前和 game/profile 写锁内重建事实后各验证一次。
  manifest/recovery record 内容变化即使聚合计数相同也会使旧 token 失效；blocked preview 不签发
  token。
- install/reinstall 的 token 还绑定 app-level prerequisite decision 的 status、stable codes 与
  rules version。required missing、规则不可用/损坏或 decision 无法证明时 fail closed；
  `signature_unverified` 只作为显式 warning。CLI/Tauri lifecycle projection 不返回 prerequisite
  issue path、display message、loader config 正文或本地绝对路径。提交前最终 prerequisite
  规则读取、配置解析和 hash 在获取 game/profile 写锁前完成；写锁内只验证已封存 decision/token
  与受控写入事实，不重复长时间 prerequisite I/O。
- 四条生命周期写命令继续复用既有 application runner、InstallPlan、backup、原子 manifest、
  rollback/recovery、Task/Audit Log 与共享写锁。CLI 不接受 target/source path、manifest、backup ref
  或 recovery ref，也不提供 `--force`。
- T13-05 的 Sandbox `install batch plan|apply|result|retry` 通过批次级 operation 支持 install、
  uninstall 和 reinstall。Preview 在构造写 runtime 前只读校验，same-revision retarget 不创建 staging、
  DB、journal、Audit 或 projection；apply 才能在 capability、token、SQLite scope admission 和共享
  game/profile 写锁全部通过后复用单项事务。Production batch token 由 per-installation secret
  （app data `secrets/` 下的随机 key，损坏即轮换）签名，跨环境互不通用；secret 不进日志、
  机器输出或诊断导出。
- Batch result 只读取显式 batch/attempt。非终态查询返回 `0` 以保留诊断能力；terminal 状态复用
  apply/retry 的稳定退出码，其中 `completed_with_errors` 返回 partial exit `5`。遗留 active attempt
  继续使 apply/retry/new apply 对同 scope fail closed。
- CLI 首次 Ctrl+C 通过 `TaskManager` 请求协作式取消；只有确认取消才发唯一 cancelled terminal 并
  返回 130。第二次中断强制退出前提示通过 recovery/status 重验状态；commit barrier 生效后不伪造
  cancelled。
- CLI-3A 在 `hmm-ports` 定义三类跨进程 scope：`background-registration-write`、
  `save-profile-write` 和 `game-profile-write`。scope identity 只接受固定全局值或稳定 game/profile ID，
  不接受路径、Steam ID、task XML、archive/manifest ref 或自由文本。
- 获取全序固定为 `background < save < game`；restore 只能按 `save -> game -> process-local game mutex`
  进入短 commit。逆序、同 scope 重入、timeout、取消和平台不可用全部 fail closed 为稳定
  `write_admission_*` code。
- Windows named mutex 的对象名和 Unix lock filename 只包含 domain-separated digest。Unix 实现从已打开
  app-data capability 以 no-follow 相对打开 lock root/file，路径被 symlink 替换时不能逃逸；owner record
  只是 stale evidence，不能删除或抢占活跃 OS lock。
- 取得跨进程 guard 只表示取得执行时隙，不是写授权。InstallPlan、manifest、backup、rollback/recovery、
  save settings/transaction、containment 与 owned Scheduled Task read-back 必须在 guard 内重验。
- CLI-3A 不改变 Production parser/runtime 门禁。Production 写命令只有在后续 CLI-3B 按 command 完成
  capability、token、Audit、锁内事实和 disposable Windows 验收后才能逐项开放；不得提供 debug 或
  环境变量绕过。
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

外部 MOD 接管（#286 adopt）是这条基线上唯一**不经安装执行器**的清单写入，其备份、回滚与失败
恢复策略如下：

- 只写安装清单，不复制、不删除、不覆盖任何游戏文件，因此**不建备份**——被接管的文件本来就不是
  本工具写上去的，没有可还原的原版；条目以 `adopted` 标记来源、`backup_ref` 为空。卸载沿用
  「无 backup_ref → 删除」，删除即删除，接管确认前必须向玩家说明。
- 唯一落盘是一次原子清单保存；它之前的每一步失败都没有中间态，所以不需要 recovery 记录；它失败
  即清单未变，可直接重试。审计写入失败不改写成功事实，以显式降级码报出。
- 写入前在 game/profile 写锁内复核：磁盘指纹与扫描记录一致、以当下清单重算的可认领集与玩家确认的
  预览一致，任一漂移拒绝并要求重扫；含读不到的文件时整次拒绝，不在残缺事实上建清单。

## 存档安全基线

存档备份和恢复必须满足：

- 默认备份目录不在游戏安装目录内。
- 支持玩家自定义备份目录。
- 备份结果写入 manifest。
- 备份源根和递归子项必须通过 no-follow metadata 拒绝 symlink、junction 与其他 reparse point；任何
  link/reparse 都不得让扫描或归档读取源根外文件。
- 恢复来源只能是后端按 `(gameId, profileId, backupId)` 精确读取的 completed backup + manifest；前端不得
  提交 archive、manifest、目标路径、文件列表或 hash。
- 恢复 preview 与任务启动都必须校验 archive/manifest identity、SHA-256、逐文件 size/hash、安全相对
  路径、大小上限和 containment，并对游戏运行中或运行状态未知 fail closed。
- 恢复前二次确认；默认开启 Profile 级 pre-restore 安全备份。用户关闭时必须显示高风险警告并额外确认，
  不能由单次请求临时关闭持久安全设置。
- pre-restore backup 必须先完整写入独立 `pre-restore/` 目录、manifest 和历史记录，之后才允许提交；普通
  retention 不得删除该目录的记录。
- 按数量、年龄或空间执行普通 retention 时，物理删除前必须先持久化清理意图；archive 与 manifest 只能
  通过 repository 目录快照和 capability-relative no-follow 句柄复验、删除。半删必须记录为可重试
  `retention_pending` / `retention_partial`，不能继续显示为 completed 或伪报释放空间。
- 空间预算计入受保护 `pre_restore` 占用，但普通 retention 不得突破该保护点或最新普通备份下限；无法
  收敛时返回稳定 blocked/partial 结果。
- 同 game/profile 的备份任务、自动/显式 retention 与恢复任务必须共享存档维护 scope。恢复从 queued 登记
  到 terminal 持有该 scope，防止来源在校验、准备、pre-restore 或提交之间被并发 retention 删除；错误、
  abort 和 panic 路径必须释放占用。该 scope 不替代目标目录的短游戏写锁。
- archive 校验、解压、staging 与安全备份位于共享写锁外；同 game/profile 的目标目录交换、rollback 和
  recovery 收尾必须串行。commit 前重新读取短事实并复核 token、目标和 staging 摘要。
- 恢复使用持久事务和受控 sibling 目录交换。失败优先恢复原 rollback sibling；无法证明原状态时保留
  事务与 recovery evidence，并返回 `save_restore_recovery_required`，不得逐文件覆盖或静默删除证据。
- 目录交换成功后必须先持久化非终态 `Committed` 事实，再幂等清理 rollback/failure evidence；只有收尾
  成功后才能持久化 `Completed`。收尾失败必须保留可重试 evidence、持久化 `RecoveryRequired` 并阻断
  新恢复，不能把“玩家文件已提交”误写成已回滚或普通失败。
- durable `Completed` 后的 Task/Audit 写入失败只能投影 evidence degradation，不能伪造玩家文件回滚或
  业务失败。
- 协作取消只有在取消终态成功持久化后才能清理 prepared staging。若终态落盘失败，必须保留 staging 与
  未完成事务、投影 `save_restore_recovery_required`，并覆盖先到达的乐观 cancelled UI 事件。
- restore commit/finalize 依赖进程内保留的父目录 capability 和目录 identity。应用在提交后、durable
  `Completed` 前崩溃或重启时不能按绝对路径重建该 capability；必须保留非终态事务与仍存活的磁盘
  evidence 并 fail closed。若崩溃发生在幂等清理过程中，部分 sibling 可能已经安全删除，后续仍须由受控
  恢复能力或人工支持根据事务与剩余 evidence 处理，不能放行新的恢复。
- 应用完全退出必须在 restore admission scope 空闲时原子关闭新 restore 登记；任一 queued/running restore
  或 scope 读取失败都必须 fail closed，恢复主窗口并拒绝完全退出。该状态不能使用后台保护 override 绕过，
  用户只能返回应用或收起到托盘，直到 restore terminal 与 evidence 收尾完成。
- 自动备份间隔和保留策略可配置。
- 备份中心只接收短 game/profile/backup identity 和规范化备注；不得接收或返回 archive/manifest 路径、
  Steam ID、hash 列表或真实存档内容。确认过的 Steam 账号快照只用于展示，不参与 restore ownership。
- 测试不得默认使用真实存档目录。

## 响应原则

- 能确认的问题会尽量复现并评估影响等级。
- 修复完成后再公开必要细节。
- 如果问题来自用户本地环境或第三方 Mod 本身，会尽量说明边界和缓解方式。
- 涉及误删、覆盖、存档损坏的报告会优先处理。
