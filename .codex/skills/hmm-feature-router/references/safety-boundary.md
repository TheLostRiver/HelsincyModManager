# 文件、安装与并发安全边界

处理压缩包、文件写入、安装、卸载、备份、存档、回滚、任务并发、日志、诊断、loader、DLL 检测、retarget staging 或 Tauri 文件命令前读取本文件。

主要源文档：

- `SECURITY.md`
- `docs/ARCHITECTURE.md`
- `docs/LOGGING.md`
- `docs/TESTING.md`
- `docs/mod_installation_strategy.md`
- `docs/ARMOR_RETARGET_DESIGN.md`

## 文件安全

- 永远不信任 Mod 压缩包中的路径。
- 拒绝路径穿越、绝对路径、符号链接/目录联接陷阱、可疑文件类型、压缩炸弹、大小写不敏感碰撞。
- 只解压到 sandbox/cache。
- 导入的原始包只读。
- 生成的变体放在 staging，可丢弃并重建。
- 真实游戏目录写入必须走 plan/manifest/backup/rollback 流程。

## 安装安全

真实写入必须遵循：

```text
analyze / preflight
  -> InstallPlan
  -> 持久化 Planned recovery intent
  -> 读取 source/target 并建立 backup
  -> 持久化 Committing rollback facts
  -> commit 玩家文件
  -> 原子保存最终 manifest
success -> 标记 Completed 并清理 recovery
failure -> rollback；rollback 失败则保留 RollbackRequired
```

卸载必须基于 manifest，不能根据当前包内容猜测。

覆盖前必须备份。失败时应尽量回滚，并留下足够状态供恢复扫描。

## Retarget 安全

- Replacement target 是一等数据，不是前端字符串 hack。
- Core 将游戏专属 metadata 当作不透明数据。
- MHW adapter 负责 `plNNN_VVVV`、`f_equip`、`m_equip`、`nativePC`、Unicode catalog 归一化和结构化 slot 替换。
- Retarget 只能替换解析出的 slot 段，不能做宽泛字符串替换。
- 冲突检查基于最终目标路径，而不是原始包路径。

## 并发

- 扫描、hash、解压、分析可以并行。
- 同一游戏实例的写入串行。
- 同一 profile 的启用/禁用串行。
- 不要在长时间解压、hash 或分析期间持有游戏写锁。
- 长任务和进度事件必须携带 `task_id`。

## 日志与隐私

- 记录结构化操作结果、task id、game id、profile/mod id、hash、大小和错误分类。
- 不记录完整本地路径、Windows/Linux 用户名、Steam ID、token、cookie、真实存档内容或第三方 Mod 内容。
- 诊断导出必须由用户主动触发，并先脱敏。

## 测试数据

使用人工构造的最小包、临时目录、fake file system 或 mock。自动测试不能要求真实 MHW 安装、真实存档或第三方 Mod 压缩包。
