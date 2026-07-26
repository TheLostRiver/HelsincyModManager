# 自主迭代路线图

本文件是**无人值守自主迭代**的任务队列。它与 [路线图](ROADMAP.md)（产品阶段）和
[任务总纲](../TODO.md)（长期功能规划）分工不同：那两份描述"要做什么产品"，
本文件描述"在没有人盯着的情况下，可以安全推进哪些具体工作"。

创建时间：2026-07-27
基线：`main == 9a8e665`

---

## 选任务的唯一标准：能否无人自证

本队列里的每个任务都必须满足：**修复的正确性可以由测试、类型检查、lint、构建、
逐字节等价或变异验证证明，不依赖任何人去看界面。**

这条标准不是保守，是有代价换来的教训。此前的迭代中反复出现同一种失败模式：

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

### 分档

| 档位 | 任务 | 完成后 |
|------|------|--------|
| **自动档** | A1 – A3 | CI 全绿且 review 意见处理完毕后，rebase 合并并 fast-forward `main` |
| **待审档** | A4 – A6 | 完成实现、开 PR、**停在这里等待人工 review，不得自行合并** |

待审档的三项都改动 `policy/project-policy.json` 或 `scripts/check-*`，属于
[工程治理与强制约束](GOVERNANCE.md#治理文件变更规则) 定义的"修改规则本身"。
该文档要求这类改动"必须被视为修改规则本身"并接受人工 review。
无人值守时没有 reviewer，因此只做到 PR 为止。

### 顺序

按 A1 → A6 顺序执行。A1 有时效性（见下），其余按序即可。
一个任务未合并（自动档）或未开 PR（待审档）前，不开始下一个。

### 每个任务的交付形态

独立 `hy/` 分支 + 独立 worktree + 独立 PR。任务内部按清晰边界拆成多个提交，
每完成一个可独立验证的子步骤就提交一次，不要攒到最后一次性提交。

---

## A1 · 把 `state.rs` / `dto.rs` 的内联测试外置

**优先级**：最高，有时效性
**风险**：medium（落点是装配全部服务与写锁的组合根）
**预计**：2 个 PR（两个文件各一个）

### 问题

`policy/project-policy.json` 对 `.rs` 设了 **2200 行**的硬性阻断上限，超过会让
`verify.ps1`、pre-commit、pre-push 和 GitHub `Verify` 作业**同时失败**。

当前：

| 文件 | 行数 | 距硬门禁 |
|------|------|----------|
| `src-tauri/src/state.rs` | 2169 | **31 行** |
| `src-tauri/src/dto.rs` | 2133 | **67 行** |

这两个恰恰是最高频改动的文件：`state.rs` 是装配所有服务的组合根，
`dto.rs` 承载全部前后端 DTO。**下一个新增 command / DTO / AppState 字段就可能把文件推过
2200，阻断整条流水线。** 在无人值守场景下这会直接卡死后续所有任务。

### 修法

两文件都带着大段内联 `mod tests`，而仓库已有成熟的外置约定
（`#[cfg(test)] #[path = "..._tests.rs"] mod tests;`）：

- `state.rs:1639` 自身已用该模式外置了 `state_core_mod_lifecycle_tests.rs`
- 另有 `install.rs:1190`、`install_recovery.rs:1824`、`lib.rs:212`、
  `mod_library_query.rs:471`、`reinstall_task.rs:661` 等多处同款

照此把 `state.rs` 的内联 `mod tests`（约 1643 起）搬到 `state_tests.rs`，
`dto.rs` 的（约 1416 起）搬到 `dto_tests.rs`。**生产代码与测试断言逐字不动，仅物理迁移。**
迁移后约为 1642 / 1415 行，远离硬门禁。

### 必须注意

- `dto.rs` 顶部有 `#[cfg(test)] use hmm_app::InstallManifestStatus;` 这类 **test-only import**，
  测试外置后要一并迁移，否则触发 unused-import 告警。
- `state.rs` 的测试里包含并发锁测试（如 `recovery_scan_waits_for_shared_game_profile_write_lock`），
  迁移后必须确认它**仍被发现并执行**，不能只看"测试没报错"——测试文件没被加载也不会报错。

### 验证

```
cargo test -p hmm-tauri        # 断言测试总数与迁移前一致，锁测试仍在其中
cargo check -p hmm-tauri
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-file-size.ps1
```

迁移前先记录 `cargo test -p hmm-tauri` 的测试条数，迁移后逐条比对。零行为变化。

---

## A2 · 清除 reinstall 提交/回滚路径上已失效的 `dead_code` 抑制

**风险**：medium（落点是安装重装/回滚引擎）
**预计**：1 个 PR

### 问题

`crates/hmm-app/src/reinstall_commit.rs:1` 有**文件级** `#![allow(dead_code)]`，
注释写着「Task 6 runner will call this crate-internal prepared commit seam」；
`reinstall.rs:869/877/884` 三处 `#[allow(dead_code)]` 注释写着「deferred to Task 6」。

**Task 6 早已完成并接线**，这些抑制的理由已经过期：

- `reinstall_task.rs:240` 与 `:407` 实际调用 `.commit(...)`
- `state.rs:111-112 / :574 / :619-620` 把 `ReinstallTaskRunner` / `ReinstallTaskService`
  装配进 `AppState`
- `lib.rs:154` 注册 `start_reinstall_task` command
- `install_recovery.rs:17` 使用 `cleanup_reinstall_transaction` / `promote_manifest_snapshots`

文件级 blanket allow 会对整个 868 行的提交/回滚模块**永久关闭 dead-code 检测**——
将来真出现未读字段或死路径也不会被发现，而这正是治理要求格外谨慎的区域。

### 修法

删除该文件级 `#![allow(dead_code)]` 与三处过期的 `#[allow(dead_code)]`（连同失效注释）。

### 必须注意

**这不是"逐字节不变的纯清理"。** 移除 allow 后若 clippy 报出确实 never-read 的字段，
删字段是对回滚数据结构的**结构性改动**：必须逐个确认该字段确非序列化所需、
确非 manifest/recovery 契约的一部分，**不得盲删**。

若出现无法确信可删的字段：保留一个**精确到该字段**的 `#[allow(dead_code)]` 并写明理由，
而不是恢复文件级 blanket allow。把这种情况记入 `findings.md`。

### 验证

```
cargo clippy -p hmm-app --all-targets -- -D warnings
cargo test -p hmm-app
cargo clippy --workspace --all-targets -- -D warnings
```

---

## A3 · 契约文档补齐 8 个已上线命令 + 防回归测试

**风险**：low
**预计**：1 个 PR

### 问题

`docs/FRONTEND_BACKEND_CONTRACT.md` 自称是"统一 Tauri command 的命名、参数、返回值和错误结构"的
长期架构契约，并逐族记录了几乎所有命令族。但 T3（Mod 元数据编辑）与 T4（分类 CRUD）的
**8 个命令整族缺席**：

```
create_category  update_category  delete_category  list_categories
set_mod_categories  get_mod_categories
update_mod_metadata  delete_mod_metadata
```

对文档逐个 grep 这 8 个命令名：全部 0 命中；其余 74 个已注册命令均有出现。
而它们前后端全链路早已接通（`lib.rs:176-183` 注册，
`category_commands.rs` / `mod_metadata_commands.rs` 定义，
`categoryApi.ts` / `modCategoryApi.ts` / `modMetadataApi.ts` 消费），
`TODO.md` 也把 T3/T4 标记为已完成且交付含"前端 typed API"。

### 修法

补两族命名条目与逐命令 params / returns / 错误码小节。
**参数与错误码必须照抄真实签名，不得臆造** —— 逐个对照
`src-tauri/src/category_commands.rs`、`src-tauri/src/mod_metadata_commands.rs`
与前端 `*Api.ts`。

同时新增一个防回归测试：解析 `lib.rs` 中 `generate_handler!` 的命令清单，
断言每个命令名都出现在契约文档中。这条护栏能防止将来再出现整族缺席。

### 验证

```
node --test src/**/*.test.mjs      # 新增的契约覆盖测试
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-doc-links.ps1
```

新测试必须做**变异验证**：临时从文档中删掉一个命令名，确认测试报错；随后还原。

---

## A4 · 文件大小门禁补字节与单行长度上限 ⚠️ 待人工 review

**风险**：medium（治理变更）
**预计**：1 个 PR，**完成后停在 PR，不自行合并**

### 问题

防止单文件无边界膨胀的唯一强门禁**只统计换行符数量**。任何把代码字节压进极少行的文件
（minify、超长 JSON/字符串、内嵌 data-URI）都能以任意大小畅通无阻。

`policy.fileSize.block` 全部是按行的行数上限，没有任何字节上限或单行最大长度限制。
CI 走的 `check-policy.mjs` 与本地 `verify.ps1` 走的 `check-file-size.ps1` 用的是同一种按行逻辑，
因此两条路径**同时**被绕过。

**这不是理论风险** —— 诊断页此前正是把整页 CSS 压成 5 行（其中一行约 3000 字符）
以通过该门禁，代价是无法维护。该实例已在近期修复，但机制漏洞仍在。

### 修法

在 policy 增加 `fileSize.blockBytes` 与可选 `maxLineLength`，
在 `check-file-size.ps1` 与 `check-policy.mjs` 中**同步**实现。

阈值必须设得足够宽以不误伤合法文件（`Cargo.lock` / `pnpm-lock.yaml` 等进 allowlist），
但足以挡住 minify 规避。当前仓库最长源码行为 1926 字符
（`docs/MOD_PREVIEW_IMAGE_PIPELINE_DESIGN.md`），次为 1715 / 1379，
均为 docs 的表格或散文；阈值若与之冲突应豁免 docs 而不是抬高全局阈值。

### 必须注意

`.ps1`（本地）与 `.mjs`（CI）是两套独立实现，**必须保持行为等价**，
否则会造成 CI 与本地门禁分叉——这本身就是新的治理缺陷。

### 验证

新增 `node --test`：构造一个约 300KB 但行数合法的 fixture 断言失败；
构造一个 3000 字符逻辑压缩成 5 行的 fixture 断言失败；
构造正常文件断言通过。随后跑一次完整 `verify.ps1` 确认无既有文件被误报。

---

## A5 · secret 强制扫描扩展名补 `.py` / `.sql` ⚠️ 待人工 review

**风险**：medium（治理变更）
**预计**：1 个 PR，**完成后停在 PR，不自行合并**

### 问题

secret 扫描按硬编码文本扩展名清单生效，该清单**缺 `.py` 与 `.sql`**。

而 policy 专门配置了 `secretScan.forceIncludePathPatterns = ['.codex/**']`
来对上下文管理目录强制扫描——因为 `AGENTS.md:39` 明令禁止在 `.codex` 内写入真实
token、会话日志、私有路径。但该目录下的 **11 个 `.py` 钩子/技能脚本因扩展名不在清单而被默认排除**，
恰恰是最可能夹带 token 或私有路径的文件类型。

另有 10 个 `src-tauri` 下的 `.sql` 迁移既不在 secret 扫描范围，也不在任何 `fileSize` 限制内。

### 修法

在两处扫描器的 `textExtensions` 中同步加入 `.py`、`.sql`；
顺带补 policy `fileSize.extensions` 的 `sql` 类别。

### 验证

新增 `node --test`：临时在 `.codex/` 下写入含伪造 token 的 `.py` fixture，
断言 `checkSecrets` 能命中；`.sql` 同理。对现存的 11 个 `.py` / 10 个 `.sql`
跑一次扫描确认无历史泄漏（作为回归基线）。

---

## A6 · CODEOWNERS 与 `policy.governanceFiles` 对齐 ⚠️ 待人工 review

**风险**：medium（治理变更）
**预计**：1 个 PR，**完成后停在 PR，不自行合并**

### 问题

治理文件变更告警（`check-governance-changes.ps1`）只读取 `policy.governanceFiles`，
而该清单：

- **缺 `.github/CODEOWNERS` 自身**
- 只有 `policy/project-policy.json` 单文件，不是 `policy/**`
- 只有 `docs/release` 而非该目录下的具体文件

因此修改 `CODEOWNERS` 本身、或 `policy/` 与 `docs/release/` 下的**其他**文件时，
本地治理告警完全不触发。

而 `CODEOWNERS` 文件自带注释要求「Keep this in sync with policy/project-policy.json
governanceFiles」，`docs/GOVERNANCE.md:173-188` 的治理文件清单也明确列出 `.github/CODEOWNERS`。
三处应当一致，实际漂移。

### 修法

在 `policy.governanceFiles` 补 `.github/CODEOWNERS`，
把 `policy/project-policy.json` 改为 `policy/**`，
把 `docs/release` 改为 `docs/release/**`。

### 验证

新增小测：断言 `CODEOWNERS` 每条路径前缀都能被某条 `policy.governanceFiles` glob 覆盖。
再故意暂存一次 `CODEOWNERS` 改动，确认 `check-governance-changes.ps1 -Mode staged`
把它列进告警。

---

## 不要做的事

以下项目**已经过评估并明确排除**，不要重新发现、不要自行开工：

| 项 | 排除理由 |
|---|---|
| 任何纯视觉美化 | 无法无人自证，见本文开头 |
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

全部任务完成后**不要自行寻找新任务开工**。改为：

1. 在 `findings.md` 中汇总本轮完成情况、遇到的问题、以及执行过程中发现但未处理的线索
2. 把新发现的候选追加到本文件末尾的「候选池」小节，标注是否可无人自证
3. 停下等待人工决策

### 候选池

（执行过程中发现的新线索追加到这里，不要直接开工）
