# SAVE-04 玩家存档恢复验收

## 结论

状态：`certified`

验收日期：2026-08-15

验收在 disposable Windows Sandbox 中完成，使用人工构造的 synthetic save/backup fixture。未读取或写入
真实玩家存档、Steam userdata、游戏目录或第三方 Mod 包。

## 候选产物

验收候选版本：`0.1.0-alpha.0`

候选提交：`5b06cef`

主要产物 SHA-256：

| 产物 | SHA-256 |
| --- | --- |
| NSIS 安装器 | `D93D2ADAE5A1F173A61D91FD726B911D4648C051F468D4B1DB18146ED6B0DC56` |
| `hmm-tauri.exe` | `937BE1444FA1B2EECEB6470ABF5FC5D98115078F508AE0903D952391878D8C6C` |
| `hmm-save-backup-worker.exe` | `53061649DBA2095ECEB2EA5D5D26B80F2D27C2EC891B80D151B9D3CAB18D30E9` |
| `hmm-save-backup-installer-cleanup.exe` | `6D9561E113925A6C051FF0534F912847456200A49CA6C0D49822E8E97BD9F26F` |

## 验收矩阵

| 场景 | 结果 |
| --- | --- |
| NSIS 安装 payload | GUI、backup worker、installer cleanup helper、uninstaller 四项齐全 |
| 初始 Profile | synthetic Profile 激活；存档目录和备份目录设置成功；恢复前安全备份默认开启 |
| 普通手动备份 | 1 个 archive、1 个 manifest；初始 marker 为 `SAVE04_ORIGINAL` |
| 恢复预览 | 显示 2 个文件、约 64 MiB，确认按钮可用，保护状态正确显示 |
| active restore 期间完全退出 | 只显示“收起至系统托盘”和“返回应用”；没有完全退出/仍然退出；HMM 进程保持运行 |
| 恢复前安全备份 | 每次恢复先写入独立 `pre-restore/`，失败前不覆盖当前存档 |
| 最终恢复 | 4 个 archive、4 个 manifest、3 个 pre-restore archive/manifest；marker 恢复为 `SAVE04_ORIGINAL` |
| 完全退出 | 恢复终态后 `hmm-tauri` 与 `msedgewebview2` 均退出 |
| 重启持久化 | 活动 Profile、存档/备份路径、备份历史和 `SAVE04_ORIGINAL` 均保留 |

第二次恢复时最终短暂出现 `SAVE04_MUTATED`，经逐个读取 manifest 与 ZIP 内容确认：用户选择的是第一次
恢复前生成的 mutated 保护点，而不是最早的手动 original 备份。该结果属于正确的历史选择语义，不是恢复提交
缺陷；再次选择最早的手动备份后最终 marker 正确恢复为 `SAVE04_ORIGINAL`。

## 自动化与残余风险

实现提交前已通过 SAVE-04 聚焦测试、完整 `scripts/verify.ps1`、workspace Rust tests/check/Clippy、
前端 typecheck/lint/test 和 findings-first 安全审查。仍接受一个 Windows 平台限制：恢复 evidence 私有目录
清理存在极短的按名删除窗口，影响范围限于 recovery evidence cleanup，分级为 Moderate；目标存档目录交换
不依赖该清理原语。

SAVE-04 认证只覆盖 synthetic disposable 环境中的产品工作流，不代表 Steam Cloud、真实玩家数据迁移或
跨设备同步能力。后续 SAVE-05 retention/备份中心已于 2026-08-16 完成认证；下一 `ready` 单元为 CLI-3A
跨进程 admission。
