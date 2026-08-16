# 跨进程写入 Admission 设计

## 目标与边界

CLI-3A 为桌面 GUI、Sandbox CLI、未来 Production CLI 与固定 `--once` worker 建立同一套跨进程写入
admission。它只解决不同 HMM 进程之间的互斥、超时、取消和 owner 崩溃释放，不开放 CLI-3B 的任何
Production 写命令，也不替代现有领域安全链：

- install/reinstall/retarget 仍使用 prerequisite decision、sealed plan、backup、manifest、rollback 与
  recovery；
- save backup/retention/restore 仍使用 profile settings、manifest、pre-restore backup、持久事务与
  recovery evidence；
- background registration 仍使用 owned task marker、精确 read-back 与 Audit；
- T13 batch 的 SQLite active-attempt admission 继续存在，但只保护 batch journal。

自动测试只允许 temp/artificial roots 与受控子进程，不访问真实游戏、Steam userdata、玩家存档或
Windows Scheduled Task。

## 当前实现状态

CLI-3A 的 port、Windows/Unix 平台实现、共享 runtime composition、三类写路径接入、稳定错误映射和
自动化已经落地，并于 2026-08-16 完成认证。本地完整 `scripts/verify.ps1`、findings-first 全 diff
审查、Ubuntu required CI run `31910573714` 与 disposable Windows synthetic 多进程 gate 均已通过，
未发现 Critical 或 Important 问题。Windows gate 覆盖 helper timeout/cancel/abandoned owner、CLI
game scope 竞争与释放、GUI/worker save scope busy fail-closed 与释放后备份增长、background
registration enable/disable 双向竞争，以及最终 task/backup/evidence 无残留。Production CLI
parser/runtime 门禁保持不变；CLI-3A 不等于 CLI-3B 的 command-level 写授权。

## Scope 模型

`hmm-ports` 定义窄 `CrossProcessWriteAdmission` port 与三类 scope：

| scope | identity | 保护对象 |
| --- | --- | --- |
| `background-registration-write` | 固定全局 identity | 后台保护 enable/disable、owned task register/unregister |
| `save-profile-write` | `game_id + profile_id` | 手动/自动备份、retention、restore 全维护生命周期 |
| `game-profile-write` | `game_id + profile_id` | install、uninstall、reinstall、retarget、recovery 与 restore 短 commit |

scope 不接受路径、Steam ID、worker 参数、task XML、archive/manifest ref 或任意自由文本。平台对象名和
lock filename 只包含 domain-separated SHA-256 digest，不暴露原始 identity。

## 固定获取顺序

跨进程 scope 全序为：

```text
background-registration-write
  < save-profile-write(game_id, profile_id)
  < game-profile-write(game_id, profile_id)
```

同一 scope rank 的多个 identity 按稳定 `(game_id, profile_id)` 顺序获取。实现记录当前 thread 已持有的
order key，拒绝逆序、重复 scope 和非 LIFO release。

当前唯一同时使用两个 scope 的流程是存档恢复：

```text
process-local save maintenance reservation
  -> save-profile-write
  -> validate / stage / optional pre-restore backup
  -> game-profile-write
  -> process-local game/profile mutex
  -> revalidate short facts and commit directory exchange
```

安装流程保持长任务在 game scope 外：

```text
scan / hash / analyze / prerequisite recheck
  -> game-profile-write
  -> process-local game/profile mutex
  -> sealed plan + manifest/recovery + containment revalidation
  -> short commit
```

后台注册只获取固定 background scope，不在持锁时等待 save/game scope。scheduler due lease 在 save scope
之外只负责 claim，不可替代通用 save 写互斥。

## 平台实现

### Windows named mutex

Windows 使用：

```text
Global\\HelsincyModManager.WriteAdmission.v1.<digest>
```

digest 输入包括当前用户 SID、canonical app-data namespace、scope kind 与稳定 scope identity。使用默认
Windows DACL，避免把跨用户访问权限扩张为公开对象。等待以短 slice 调用 `WaitForSingleObject`，每轮检查
deadline 与 `CancellationToken`：

- `WAIT_OBJECT_0`：正常获取；
- `WAIT_ABANDONED`：OS 已释放死亡 owner，返回 recovered-owner evidence 后继续正常重验；
- deadline 到达：`write_admission_busy`；
- cancellation：`write_admission_cancelled`；
- 其他平台错误：稳定 `write_admission_unavailable`，不输出对象名或原始错误。

guard 保持 thread-affine 且非 `Send`，公开 trait object 与内部线程亲和 marker 共同阻止跨线程移动。
Drop 在 owner thread 调用 `ReleaseMutex`；失败只写脱敏安全日志，不能把已经提交的玩家文件事实改写成
回滚或业务失败。handle 关闭后 OS 仍负责最终释放。

### 非 Windows advisory lock

Ubuntu CI 与未来 Unix 平台使用 canonical app-data root 下固定 `write-admission/` 目录。初始化时持有
app-data `Dir` capability；每次 acquisition 都通过该 capability 以 no-follow 方式重新打开 lock root，
最终 lock file 也以 capability-relative `FollowSymlinks::No` 打开。即使原路径随后被 symlink 替换，
创建/打开也不能逃逸到外部目录。每个 scope 使用 digest 文件名，通过 `fs2` exclusive advisory lock
提供 OS 级跨进程 authority，并在 acquisition deadline 内轮询 `try_lock_exclusive`。

持锁后写入不含路径和用户信息的 bounded owner record；正常 Drop 先清空并 sync，再 unlock。进程退出时
OS 自动释放 file lock，残留非空 owner record 只表示 stale-owner recovery evidence。metadata 不是锁，
不能仅凭 PID、时间戳或文件内容删除/抢占活跃 owner。

## 共享装配与接入点

`HmmRuntime::from_builder` 创建一个共享 admission coordinator，并注入：

- install、uninstall、reinstall、retarget 与 install recovery runners；
- install recovery scan/preview 的一致性读取边界；
- save backup runner、save retention、save restore runner；
- background registration service；
- 固定 `--once` worker 使用的同一 save backup runner。

应用层测试构造器可以显式使用 process-local fake；完整 runtime 不允许静默回退。Production CLI policy
仍为 `disabled`，Sandbox policy 仍为 `sandbox_only`。CLI-3A 完成不改变 parser command tree。

## 锁内重验

获得跨进程 guard 不代表授权。所有写路径在 guard 内继续执行其现有最终重验：

- install/reinstall/retarget：prerequisite/preview token、sealed plan、manifest/recovery、target identity、
  source/containment；
- uninstall/recovery：expected revision/digest、manifest/recovery 与 owned target facts；
- backup/retention：活动 profile、save/backup settings、source/target identity、repository facts；
- restore：validated source、transaction、prepared staging、target identity 与 game-running short facts；
- background registration：当前 desired settings、owned task marker、exact read-back。

abandoned/stale owner recovery 不跳过任何重验，也不自动删除 manifest、recovery、backup、task 或数据库
记录。

## 错误与日志

稳定 code 至少包括：

- `write_admission_busy`
- `write_admission_cancelled`
- `write_admission_order_violation`
- `write_admission_unavailable`

日志只记录 scope kind、结果、等待毫秒和 recovered-owner 类型；禁止记录 mutex 名、lock path、SID、完整
app-data path、Steam ID、存档路径或原始平台错误。释放失败只表示证据降级，不改变已经完成的提交、回滚
或恢复事实。

## 自动化矩阵

| 场景 | 断言 |
| --- | --- |
| 两个独立进程竞争同一 scope | 恰好一个持有；另一个在 deadline 返回 busy |
| 不同 profile/scope | 不错误互斥 |
| cancellation | waiter 在 bounded slice 内返回 cancelled |
| owner 无 Drop 退出 | 后续进程可获取并报告 abandoned/stale recovery |
| 逆序与同 scope 重入 | 获取前稳定拒绝，不等待 |
| Windows mutex / Unix lock 错误 | fail closed，输出不含原始 name/path |
| install/backup/background fake admission | busy 时不进入领域写入；成功时锁内重验仍执行 |
| runtime composition | GUI/CLI/worker 引用同一 admission coordinator |
| Production CLI gate | policy/parser 继续拒绝全部 Production 写命令 |

候选阶段已运行 touched crate 聚焦测试、真实双进程测试、完整 `scripts/verify.ps1` 与 findings-first 全
diff 自审。2026-08-16 disposable Windows gate 已单独验证 GUI/CLI/worker 竞争、timeout、进程强退后的
abandoned 获取、释放后正常写入和后台注册 scope；最终 `gate-final=ready-for-review`，owned task 为
`Ready`、archive/manifest 为 `3/3`、live gate process 为 `0`。人工步骤只使用 synthetic fixture，
不把普通 CI 结果替代安装态验收。Unix capability-relative file-lock 分支由 Ubuntu required CI
实际运行，不能用 Windows named mutex 结果代替。
