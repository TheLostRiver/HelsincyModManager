# 安装安全 Checklist

用于 HMM install、uninstall、backup、rollback、staging、archive 和 save-safety 工作。

## 数据边界

- 原始导入 Mod 包保持只读。
- 解压目标是 sandbox/cache，绝不是 game 或 save 目录。
- Retarget/materialized variants 只能写入 staging。
- Staging 可以删除和重建；它不是安装事实来源。
- 真实游戏写入只能发生在 `InstallPlan` 和 conflict/dependency checks 之后。
- 默认 save backups 位于游戏安装目录之外。
- Save restore 从 manifest-backed backup 读取，并在写入前验证。

## 路径安全

- 比较 logical paths 前先 normalize separators。
- 拒绝 `..`、绝对路径、drive prefixes、UNC paths、会改变含义的空段、symlink/junction escape 和大小写不敏感碰撞。
- write/delete 前确认最终解析后的 filesystem target 仍在预期 root 下。
- Conflict detection 使用 retarget/staging 后的最终 target paths，不使用原始 archive paths。

## Install/Uninstall 链路

- Install：analyze -> build `InstallPlan` -> conflict/dependency checks -> backup -> commit -> manifest -> rollback/recover path。
- Overwrite：replacement 前备份 existing file。
- Manifest：记录足够信息，使 uninstall/recover 不需要重新读取第三方 archive。
- Uninstall：只移除 manifest-owned files；保留未知 user/game files。
- Failure：best-effort rollback；记录 rollback 成功或失败。

## Save Backup/Restore

- 支持默认 backup directory 和玩家选择的 backup directory。
- Backup results 通过 manifest 支撑，并带 hash 或等价 validation data。
- 写入 save location 前必须 restore validation 和显式确认。
- 触及时保留 automatic backup interval 和 retention settings。
- 覆盖不可写 backup directories 和 retention limits。
- 自动测试只使用 temp save directories；绝不读取或写入真实玩家 saves。

## Logging

- 真实 game directory writes 前，需要 logging/telemetry initialization、`task_id` generation/propagation、redaction helpers、log directory resolution、Audit Log writer，以及 redaction/audit events 测试。
- Game directory writes、overwrites、deletes、backup、restore、manifest、rollback 和 recovery 都需要 Audit Log。
- 可用时记录 task id、game id、profile/mod id、logical target、hash/size、result 和 error classification。
- 脱敏 full local paths、usernames、Steam IDs、tokens、cookies、real save content 和 third-party Mod content。

## 测试 Fixtures

- 使用 temp directories、fake file systems 和人工 tiny package fixtures。
- 自动测试不要要求真实 MHW:I、真实 save directories 或真实 third-party Mod packages。
- success、failure、uninstall 和 rollback 后断言 temp game directory state。
- 对任何已报告的数据丢失、escape、collision 或 partial-manifest bug 添加 regression tests。
