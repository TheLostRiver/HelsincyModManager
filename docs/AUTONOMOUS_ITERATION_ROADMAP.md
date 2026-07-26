# 自主迭代路线图

本文件是**无人值守自主迭代**的任务队列。它与 [路线图](ROADMAP.md)（产品阶段）和
[任务总纲](../TODO.md)（长期功能规划）分工不同：那两份描述"要做什么产品"，
本文件描述"在没有人盯着的情况下，可以安全推进哪些具体工作"。

创建时间：2026-07-27
基线：`main == 9a8e665`

---

## 两条必须先知道的事实

这两条经实际核实，会改变你对"验证通过"的理解。

### 一、CI 与 `verify` 都不跑前端测试，也不跑 clippy

| 门禁 | 前端 | Rust |
|------|------|------|
| `scripts/verify.sh`（CI 走这条） | typecheck / lint / build | `cargo test --workspace`、`cargo check --workspace` |
| `scripts/verify.ps1`（本地） | typecheck / lint / build | 同上 |

`package.json` 的 `test` 脚本（`node --test "src/**/*.test.mjs"`，约 400 个测试）
**没有被任何门禁调用**。`cargo clippy` 同样不在任何门禁里。

**因此「CI 全绿」不等于「测试通过」。** 前端测试与 clippy 必须由你手动运行，
并把结果作为验收证据。这条差距本身是 A6 要修的问题。

### 二、治理文件改动没有强制拦截

`scripts/check-governance-changes.ps1` 只打印黄色警告，**从不非零退出**；
它只被 `.githooks/pre-commit` 与 `pre-push` 调用，而 hooks 是可选安装、可被 `--no-verify` 绕过，
`verify.ps1` / `verify.sh` / CI 都不调用它。

**因此改动 `policy/`、`scripts/`、`.github/`、`AGENTS.md` 等治理文件时，
没有任何自动机制会拦住你或通知任何人。** 只能靠自觉。

---

## 选任务的唯一标准：能否无人自证

本队列里的每个任务都必须满足：**修复的正确性可以由测试、类型检查、lint、构建、
逐字节等价或变异验证证明，不依赖任何人去看界面。**

这条标准是有代价换来的。此前迭代中反复出现同一种失败模式：

- 修吸顶状态栏重叠时，连续三次基于静态 CSS 推理判断根因，三次都错，全靠人工截图纠正。
  真因是 `position: sticky` 的 `top` 大于元素在滚动视口中的自然偏移量时，
  **绘制位置**被下推而**布局盒不变**——量 DOM 尺寸量不出来，只有实际渲染能暴露。
- 修诊断页时把页头基础态从 `flex` 改成 `grid`，导致 `@media (max-width: 900px)` 里的
  `display: grid` 变成空操作，窄屏页头不再堆叠。全量测试、类型检查、构建**全绿**。
- 重做详情对话框时把内容区背景改成浅底，使嵌在其中的替换目标面板 6 处同色区块**整片隐形**。
  同样全绿。

结论：**纯视觉改动不进入自主队列。** 它们不是不重要，而是无人值守时无法判定成败。

---

## 执行规则

### 全部任务只做到 PR，一律不合并

无人值守期间**禁止使用 `gh pr merge`（含 `--admin`）**，也不得 fast-forward `main`。

原因：本仓库 `main` 的分支保护要求人工 approving review 与 Code Owner review，
而 `--admin` 会同时绕过人工审查**和必需状态检查**。结合上面"CI 不跑前端测试"这一事实，
自动合并等于把主干的唯一防线交给一个已知不充分的信号。

每个任务的终态是：**分支已推送 + PR 已开 + PR 正文写清验收证据**。
人工回来后逐个 review 并合并。

因此每个任务都必须能**独立从 `main` 分出**，不依赖前一个任务已合并。
下面的任务划分已经保证了这一点（同一批治理文件的改动被合并为一个任务）。

### 顺序

按 A1 → A6 顺序执行。A1/A2 有时效性（见下）。
一个任务未开出 PR 前，不开始下一个。

### 每个任务的交付形态

独立 `hy/` 分支 + 独立 worktree + 独立 PR。任务内部按清晰边界拆成多个提交，
每完成一个可独立验证的子步骤就提交一次，不要攒到最后一次性提交。

---

## A1 · 把 `state.rs` 的内联测试外置

**优先级**：最高，有时效性 · **风险**：medium · **1 个 PR**

### 问题

`policy/project-policy.json` 对 `.rs` 设了 **2200 行**硬性阻断上限，超过会让
`verify.ps1`、pre-commit、pre-push 和 GitHub `Verify` 作业**同时失败**。

`src-tauri/src/state.rs` 当前 **2169 行，距硬门禁仅 31 行**。
它是装配所有服务的组合根，属最高频改动文件——下一个新增 AppState 字段就可能推过上限，
阻断整条流水线。

### 修法

仓库已有成熟的外置约定 `#[cfg(test)] #[path = "..._tests.rs"] mod tests;`：
`state.rs:1639` 自身已用该模式外置了 `state_core_mod_lifecycle_tests.rs`，
另有 `install.rs:1190`、`install_recovery.rs:1824`、`lib.rs:212`、
`mod_library_query.rs:471`、`reinstall_task.rs:661` 等多处同款。

把内联 `mod tests`（约 1643 起）整体搬到 `state_tests.rs`。
**生产代码与测试断言逐字不动，仅物理迁移。** 迁移后约 1642 行。

### 必须注意

测试里包含并发锁测试（如 `recovery_scan_waits_for_shared_game_profile_write_lock`）。
**测试文件没被加载也不会报错** —— 因此必须先记录迁移前 `cargo test -p hmm-tauri` 的
测试总条数，迁移后逐条比对，确认锁测试仍在其中。只看"没有失败"不算验证。

### 验证

```powershell
cargo test -p hmm-tauri
cargo check -p hmm-tauri
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-file-size.ps1
```

---

## A2 · 把 `dto.rs` 的内联测试外置

**优先级**：最高，有时效性 · **风险**：low · **1 个 PR**

同 A1 的问题与修法。`src-tauri/src/dto.rs` 当前 **2133 行，距硬门禁 67 行**，
承载全部前后端 DTO，同样高频改动。测试模块约从 1416 行起，搬到 `dto_tests.rs`。

### 必须注意

`dto.rs` 顶部有 `#[cfg(test)] use hmm_app::InstallManifestStatus;` 这类
**test-only import**，测试外置后要一并迁移，否则触发 unused-import 告警。

### 验证

同 A1（`-p hmm-tauri`），同样先记录测试条数再比对。

---

## A3 · 清除 reinstall 提交/回滚路径上已失效的 `dead_code` 抑制

**风险**：**high**（落点是安装重装/回滚引擎） · **1 个 PR**

### 问题

`crates/hmm-app/src/reinstall_commit.rs:1` 有**文件级** `#![allow(dead_code)]`，
注释写着「Task 6 runner will call this crate-internal prepared commit seam」；
`reinstall.rs:869/877/884` 三处 `#[allow(dead_code)]` 注释写着「deferred to Task 6」。

**Task 6 早已完成并接线**，这些抑制的理由已经过期：
`reinstall_task.rs:240` 与 `:407` 实际调用 `.commit(...)`；
`state.rs:111-112 / :574 / :619-620` 装配 `ReinstallTaskRunner` / `ReinstallTaskService`；
`lib.rs:154` 注册 `start_reinstall_task`；
`install_recovery.rs:17` 使用 `cleanup_reinstall_transaction` / `promote_manifest_snapshots`。

文件级 blanket allow 对整个 868 行的提交/回滚模块**永久关闭 dead-code 检测**。

### 修法

删除该文件级 `#![allow(dead_code)]` 与三处过期的 `#[allow(dead_code)]`（连同失效注释）。

### 硬性约束

**这不是"逐字节不变的纯清理"。** 移除 allow 后若 clippy 报出 never-read 字段：

> **禁止删除任何字段或分支。** 改为加一个**精确到该字段**的 `#[allow(dead_code)]`
> 并写明理由，记入 `findings.md`，然后**停止本任务并在 PR 中标注需要人工判断**。

理由：删字段是对回滚数据结构的结构性改动，可能移除 manifest/recovery 契约或序列化所需的字段。
`SECURITY.md` 的红线是"游戏目录写入必须走 manifest/backup/rollback"，
一旦削弱，将来触发回滚时可能无法正确还原游戏目录——这是本项目最高危的失败面。

### 验证

```powershell
cargo clippy -p hmm-app --all-targets -- -D warnings
cargo test -p hmm-app
cargo clippy --workspace --all-targets -- -D warnings
```

---

## A4 · 契约文档补齐 8 个已上线命令 + 防回归测试

**风险**：low · **1 个 PR**

### 问题

`docs/FRONTEND_BACKEND_CONTRACT.md` 自称是"统一 Tauri command 的命名、参数、返回值和错误结构"的
长期架构契约，并逐族记录了几乎所有命令族。但 T3（Mod 元数据编辑）与 T4（分类 CRUD）的
**8 个命令整族缺席**：

```text
create_category  update_category  delete_category  list_categories
set_mod_categories  get_mod_categories
update_mod_metadata  delete_mod_metadata
```

对文档逐个 grep 这 8 个命令名：全部 0 命中；其余 74 个已注册命令均有出现。
而它们前后端全链路早已接通（`lib.rs:176-183` 注册，
`category_commands.rs` / `mod_metadata_commands.rs` 定义，
`categoryApi.ts` / `modCategoryApi.ts` / `modMetadataApi.ts` 消费）。

### 修法

补两族命名条目与逐命令 params / returns / 错误码小节。
**参数与错误码必须照抄真实签名，不得臆造** —— 逐个对照后端命令定义与前端 `*Api.ts`。

同时新增防回归测试：解析 `lib.rs` 中 `generate_handler!` 的命令清单，
断言每个命令名都出现在契约文档中。

### 验证

```powershell
corepack pnpm run test
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-doc-links.ps1
```

新测试必须做**变异验证**：临时从文档中删掉一个命令名，确认测试报错；随后还原。

---

## A5 · 治理检查加固（三合一）

**风险**：medium（治理变更） · **1 个 PR，三个提交**

这三项都改 `policy/project-policy.json` 或 `scripts/check-*`，彼此会冲突，
因此合并为一个任务、一个分支、三个边界清晰的提交。

### 提交 1：文件大小门禁补字节与单行长度上限

防止单文件无边界膨胀的唯一强门禁**只统计换行符数量**。任何把代码字节压进极少行的文件
都能以任意大小畅通无阻。`policy.fileSize.block` 全部是按行上限，无字节或单行长度限制；
CI 走的 `check-policy.mjs` 与本地走的 `check-file-size.ps1` 用同一种按行逻辑，
因此**两条路径同时被绕过**。

**这不是理论风险**：诊断页此前正是把整页 CSS 压成 5 行（其中一行约 3000 字符）以通过该门禁。
该实例已修复，机制漏洞仍在。

修法：在 policy 增加 `fileSize.blockBytes` 与可选 `maxLineLength`，
在两个检查器中**同步**实现（`.ps1` 与 `.mjs` 是两套独立实现，必须行为等价，
否则造成 CI 与本地门禁分叉，那本身就是新的治理缺陷）。

阈值须足够宽以不误伤（`Cargo.lock` / `pnpm-lock.yaml` 进 allowlist）。
当前仓库最长源码行为 1926 字符（`docs/MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md`），
次为 1715 / 1379，均为 docs 的表格或散文；若阈值与之冲突应豁免 docs 而非抬高全局阈值。

### 提交 2：secret 强制扫描补 `.py` / `.sql`

secret 扫描按硬编码文本扩展名清单生效，该清单**缺 `.py` 与 `.sql`**。
而 policy 专门配置了 `secretScan.forceIncludePathPatterns = ['.codex/**']` 来强制扫描
上下文管理目录——因为 `AGENTS.md:39` 明令禁止在 `.codex` 内写入真实 token、会话日志、私有路径。
但该目录下的 **11 个 `.py` 脚本因扩展名不在清单而被默认排除**。
另有 10 个 `src-tauri` 下的 `.sql` 迁移既不在 secret 扫描范围，也不在任何 `fileSize` 限制内。

修法：两处扫描器的 `textExtensions` 同步加入 `.py`、`.sql`；
顺带补 policy `fileSize.extensions` 的 `sql` 类别。

### 提交 3：CODEOWNERS 与 `governanceFiles` 对齐

`check-governance-changes.ps1` 只读取 `policy.governanceFiles`，而该清单
**缺 `.github/CODEOWNERS` 自身**、只有 `policy/project-policy.json` 单文件而非 `policy/**`、
只有 `docs/release` 而非 `docs/release/**`。因此修改 `CODEOWNERS` 本身或那两个目录下的
其他文件时，告警完全不触发。而 `CODEOWNERS` 自带注释要求与该清单保持同步，
`docs/GOVERNANCE.md:173-188` 的清单也列出了 `.github/CODEOWNERS`——三处应一致，实际漂移。

修法：补 `.github/CODEOWNERS`，把 `policy/project-policy.json` 改为 `policy/**`，
把 `docs/release` 改为 `docs/release/**`。

### 验证

每个提交都要有对应的 `node --test`（fixture 断言拦截生效 + 正常文件不误伤），
并对现存文件跑一次完整 `verify.ps1` 确认无既有文件被误报。
最后补一条小测：断言 `CODEOWNERS` 每条路径前缀都能被某条 `governanceFiles` glob 覆盖。

---

## A6 · 把前端测试与 clippy 接入门禁 ⚠️ 需人工确认范围

**风险**：medium（治理变更，改 `scripts/verify.*`） · **1 个 PR**

### 问题

见本文开头「必须先知道的事实」第一条：`package.json` 的 `test` 脚本（约 400 个前端测试）
与 `cargo clippy` **从未被任何门禁调用**。CI 绿灯不代表测试通过。

### 修法

在 `scripts/verify.sh` 与 `scripts/verify.ps1` 中同步加入 `pnpm run test` 与
`cargo clippy --workspace --all-targets -- -D warnings`。

### 必须注意

- 加 clippy 后可能暴露一批既有告警。**若数量可控就一并修；若数量很大，
  不要在本任务里硬扛** —— 改为先只接入 `pnpm run test`，把 clippy 接入拆成独立候选记入候选池，
  并在 PR 中说明实际告警数量。
- 加入前端测试会显著拉长 CI 时长，需在 PR 中说明。
- `.sh` 与 `.ps1` 必须行为等价。

### 验证

在本分支上跑完整 `verify.ps1`，确认新加的两步实际执行且通过；
故意让一个前端测试失败，确认 `verify` 整体非零退出；随后还原。

---

## 不要做的事

以下项目**已经过评估并明确排除**，不要重新发现、不要自行开工：

| 项 | 排除理由 |
|---|---|
| 任何纯视觉美化 | 无法无人自证，见上文 |
| T13 批量操作 | 大特性，需人工优先级评审（`TODO.md` 明确要求） |
| T20 浮层基元收敛 | 重构会动已稳定的模态框/Sheet 链路，需人工确认范围 |
| `ProfilePage.tsx` 1612 行拆分 | 超说明线但未超硬线；拆分方案影响观感，需人工定 |
| 全项目 47 处非标准字重 | 会让部分文字**看起来变细**，属可见视觉变化 |
| 改变安装/卸载/回滚/备份的行为语义 | 高风险安全链路，需完整设计与安全评审 |
| 新增 Tauri 命令或改变现有命令契约 | 属公共契约变更，需人工评审 |

以下候选在勘察中被**对抗验证推翻**，不要重提：MHW 专属规则泄漏进 `hmm-core`、
CSS 缺失 token 引用导致的对比度缺陷、Mod 右键菜单项不可达、仪表盘日志时间戳、
未被引用的 `FirstLaunchDashboard.tsx`、备份路径测试跨平台问题、
存档备份 writer 拒绝码缺测试、前端测试锁实现形态、契约文档描述不存在的命令、
README 文档索引遗漏。

---

## 队列耗尽之后

A1–A6 全部开出 PR 后**立即停止并进入空闲**。明确禁止：

- 自拟新任务并实现
- 执行下方候选池中的条目
- 回头"优化"已经开出 PR 的任务

执行过程中发现的新线索，只允许**追加到候选池文本**并停下等待人工决策。

### 候选池

（执行过程中发现的新线索追加到这里，标注是否可无人自证，不要直接开工）
