# HelsincyModManager 会话交接提示词

新开会话时，把下面「提示词正文」整段粘进去即可。文件本身留在仓库根目录方便更新，
`HANDOFF.md` 需要提交就提交，不需要就删掉。

> 本文件最后更新：2026-09-03（晚），HEAD = `99ec9b6`（PR #322 在开、未合并）。
> **可信度用 `git log` 交叉验证**：对比文档里出现的 commit 短号与实际 HEAD 的差集。
> 2026-08-30 曾发现本文件漏了 8 个提交、且 #278 状态写反（写「待做」实际已做完），
> 照旧文档接手会往错误方向做。2026-09-03 又订正一处：one001 双胞胎的
> 「名字 ↔ 沙箱」对应旧版写反了（见附录，以 `results.json` 的 `display_name` 为准）。

---

## 提示词正文

````
你接手 D:\DEV\HelsincyModManager 的 HelsincyModManager 项目。先读根目录 AGENTS.md
（简体中文回复、小步提交、收尾必须跑 verify.ps1、不声称未执行的测试通过）和
docs/ARCHITECTURE.md，再看根目录 HANDOFF.md（上一手交接文档）、
docs/TROUBLESHOOTING.md（症状速查表，遇到「报错指不到原因」先查这里）和
.workbuddy-ai/memory/ 下最近几天的日志（有踩过的坑）。

## 一、项目基本事实

- Tauri 2 + React 19 + Vite 7 + TypeScript。Rust workspace 在 src-tauri/crates/
  （hmm-core / ports / infra / app / runtime / games-mhw / cli / save-backup-sidecars），
  前端在 src/。corepack pnpm。
- 校验：`powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/verify.ps1`
  （策略检查 ×7 → 前端 typecheck / lint / test / build → cargo fmt / test / check / clippy）
  **必须全绿才能提交**。
- 提交风格：中文 conventional commits。
- `.workbuddy/`、`.workbuddy-ai/`、`.zcode/`、`tmp/`、`target/` 是本地状态，已 gitignore，
  绝不提交。

## 二、环境坑（这些是会咬人的，不是背景信息）

1. **不要在会话里跑长耗时 cargo**。工具单条命令 10 分钟上限；而
   `run_in_background` 的命令会被**强制沙箱化**（`dangerouslyDisableSandbox` 对后台无效，
   同一个 `cargo check` 前台成功、后台失败，实测过）。沙箱化的 cargo 会在 `target/` 留下
   **删不掉的 0 字节僵尸文件**，之后所有 cargo 都卡在
   `failed to open: target/debug/.cargo-lock`（os error 5）。
   绕过（同卷改名瞬时）：
   `mv target/debug target/debug-pN && mkdir target/debug`，再把
   `build deps examples incremental .fingerprint` 搬回去。
   → **verify.ps1 交给用户在本地终端跑**，别自己死磕。
   例外：`cargo check / clippy / test -p <crate>` 针对单个 crate 通常几十秒内完成
   （hmm-runtime 约 20s），可以放心跑。
2. **本机 `hmm-infra --lib` 曾稳定挂 16 个测试**（symlink 与目录删除相关），
   而用户本机跑同一份代码全绿 → 已定性为工具使用方式造成的（见 issue #281，CLOSED），
   两次 verify 全绿作证。**看到测试红先做证据链排除，别急着改代码。**
3. `nohup ... & disown` 的进程会随工具调用结束被杀，别指望挂后台。
4. Git Bash 下 `corepack` shim 路径解析坏了（拼出 `d:\c\Users\...`）。用
   `"D:/Nodejs/node.exe" "D:/Nodejs/node_modules/corepack/dist/corepack.js" pnpm <script>`。
   `verify.ps1` 内部走 `cmd /c corepack pnpm`，不受影响。
   **另：Git Bash 下 `git commit -m` 多行中文会挂**（中文标点/反引号触发 EOF），
   一律用 `git commit -F <file>`；`gh` 写长文用 `--body-file`。
5. **PowerShell 执行策略各 scope 均 `Undefined` → 等效 `Restricted`**，全局 `.ps1` shim
   （如 `pnpm`）跑不了，报 `PSSecurityException`。**一律用 `pnpm.cmd`**（走 cmd.exe）。
   别改用 Git Bash 绕开（那里 corepack 拼错路径，`beforeDevCommand` 是
   `corepack pnpm dev`，vite 起不来）。
6. **清理 dev 链进程时不要无差别杀 node**。本机 `node.exe` 里混着 WorkBuddy 自身进程、
   另一个项目 `E:\DEV\bolo-pi` 的进程和本项目 dev 链。必须按端口精确定位
   （1420 / 9223），动手前先确认端口是否真在监听。
7. dev 链：`$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9223"`
   再 `pnpm.cmd tauri:dev`（PowerShell 语法，`VAR=x cmd` 是 bash 语法会报错）。
   端口 1420 已保留，devUrl 与 vite 都钉死 127.0.0.1，不要改回 localhost。
   冷启动可能白屏 1-2 分钟（#277），等 vite CPU 平息后 Ctrl+R。
8. 应用日志按 UTC 日期滚动，根目录 `%APPDATA%\dev.helsincy.modmanager\logs\`。
   **活数据在 `hmm.db-wal`（几 MB），不在 `hmm.db`（4KB）**。
   **任务进度在 `logs/tasks/`，审计在 `logs/audit/`，二者都不是 `logs/app/`**——
   判断安装是否真的提交，要看 `logs/tasks/` 里有没有走到 `commit.processing → completed`。
9. 跑 verify 前先杀干净 dev 链（hmm-tauri + vite/tauri 的 node 进程），否则抢 cargo 锁。
10. **读不到 `hmm-tauri` 进程的环境变量**（Windows 限制，PowerShell 的
    `StartInfo.EnvironmentVariables` 为 null）。判断当前沙箱根靠反推，见第五节。

## 三、仓库硬约束（违反了测试会红，但报错信息不会告诉你原因）

- **`src/features/mods/modsLibraryData.ts` 是生成文件**。改快捷操作栏动作项等必须改
  `scripts/generate-mod-library-mock-data.mjs`，再跑
  `node scripts/generate-mod-library-mock-data.mjs --count 72` 重新生成。
- **新增 Tauri command 必须登记 `docs/FRONTEND_BACKEND_CONTRACT.md`**。
  `src/shared/api/tauriContractCoverage.test.mjs` 扫 `generate_handler!` 逐个比对。
- **i18n「无硬编码中文」清单**含 `ModLibraryPage.tsx`、`ModLifecycleFeedback.tsx`、
  `ModContextMenu.tsx`、`CompactActionPanel.tsx` 等。其 `stripComments` **只过滤整行 `//`
  注释**——行尾注释里的中文一样会被抓。这些文件一律英文注释。
- copy 字典新增 key 要三语齐备，并加进 `src/shared/i18n/i18n.test.mjs` 的字典清单。
- `verify.ps1` 用 `git diff --check` 查空白，文件末尾多一个空行直接红。
- **`pnpm test` 只跑 `src/**/*.test.mjs`**。scripts/ 下新增测试必须在
  `scripts/verify.ps1` 逐条显式登记，否则等于没写。
- **日志必须走 `emit_safe_app_log(AppLogEvent::warning(...).with_xxx())` 才会落盘**。
  普通 `tracing::warn!` 不过 app 日志层（`app_log.rs` 的 `on_event` 只处理
  target == `hmm.safe_app_log`），dev 终端看得到、`logs/app/app-*.log` 里没有。
  且字段有白名单，未知字段会让**整条事件**被拒——自定义语义要映射到既有字段
  （「阶段」放 `operation`，「子步骤」放 `phase`），别自己发明字段名。详见排障手册 6.1。
- **Production vs Sandbox**：GUI 常态是 Production，`write_available` / `preview_available`
  **只在 Sandbox 为 true**。任何不走批量生命周期框架的动作（如批量删除）
  不要用 `batchWriteUnavailableReason` 做门禁，否则 Production 下恒为死按钮。
  注意 `CompactActionPanel.batchCapabilityDisabledReason` 对批量模式下除 preview-plan 外的
  所有动作统一套这个门，豁免要改在这一层。
- **沙箱写入准入是两套语义，不要统一**（#273）：
  `SandboxRuntimeWriteAdmission`（GUI）**只校验游戏根**；
  `lifecycle_automation.rs`（批量/CLI）**仍校验 app-data + 游戏根**——
  后者的数据根就是沙箱根本身，语义正确，别顺手改。
- 既有测试用**字面量断言源码形状**（如 `batchSelectionActive && actionId === "preview-plan"`），
  重构时保留原表达式形态，否则测试红得莫名其妙。
- **路由切换会卸载页面组件**（`RouterOutlet` 退出动画后 `completeRouteExit` 移除 layer），
  页面内 `useState` / `useReducer` 的工作态——选区、搜索词、筛选、视图模式——**全部重置**。
  偏好落 localStorage、工作态不落，是既有区分（全页只有 `showCardCategoryLabels` 落盘）。
  **别把「切侧边栏丢多选」当 bug 修。** 唯一提到应用级的工作态是外部状态扫描结果的
  会话表（PR #322，`ExternalStateSessionProvider` 挂在 `RouterOutlet` 之上）——那是维护者
  验收时被绊到后专门拍板的 A+，不是先例，别照着把别的工作态也往上提。

## 四、当前进度（2026-09-03 晚，HEAD = `99ec9b6`）

最近提交（由新到旧）：

| commit | 说明 |
|---|---|
| （PR #322，未合并） | feat(mods) 外部状态会话表提到应用级 Provider，路由切换不再丢徽标（**#286 3b-2 A+**，分支 `hy/external-state-session-store`，已 verify + 真机验收） |
| `99ec9b6` | Merge PR #321（hy/external-mod-adopt-ui，**#286 adopt PR B**：详情弹窗接入接管——按钮、alertdialog 二次确认、三语文案） |
| `16c4921` | Merge PR #320（hy/external-mod-adopt-backend，**#286 adopt PR A**：后端全链路，4 个提交逐层向上） |
| `335d526` | feat(tauri) `start_external_mod_adopt` 命令 + 契约/LOGGING/SECURITY 登记 + 契约覆盖用例逐个盯稳定码 |
| `01aab2e` | feat(runtime) `ConfiguredExternalModAdopter`（锁外前置拒绝 → 准入 → 阻塞写锁 → 锁内双重验 → 提交屏障 → 原子写清单 → 审计）+ 任务服务 + 31 条集成测试 |
| `bbc9cb2` | feat(app) 认领集派生纯函数 `derive_external_adopt_plan` + `append_adopted_entries` + 扫描服务返回游戏侧摘要 |
| `f2e8931` | feat(core) 清单条目新增 `adopted` 标记（serde default，老清单逐字节不变） |
| `438926d` | Merge PR #319（**#305**，方案 A：沙箱侧读失败标成「读不到」留在原位，不再静默丢弃、不再错位） |
| `af21836` | Merge PR #318（**#309**，方案 D：外部导入物化保留路径原始大小写；存量包删了重导） |
| `5df03df` | Merge PR #317（**#286 切片 9c**：卡片徽标全占用改口「已被 X 占用」） |
| `afd1607` | Merge PR #316（docs/handoff-286-attribution，上一版 HANDOFF + 排障手册三坑） |
| `868dccd` | Merge PR #315（hy/external-state-occupier-ui，**#286 切片 9b**：弹窗展示占用归因） |
| `180e5ae` | feat(mods) 详情弹窗展示占用归因（占用提示行 + 文件行小胶囊，三语） |
| `f8dbf15` | Merge PR #314（hy/external-state-attribution，**#286 切片 9a**：归因后端事实链） |
| `95b45cd` | feat(runtime) 外部状态扫描的占用归因（stage 3 锁内读清单，claimed_by 全链路） |
| `d80a42c` | Merge PR #313（hy/library-external-badge，**#286 切片 3b-2**：卡片消费扫描结果，方案 A） |
| `3f32957` | feat(mods) 卡片状态位消费外部状态扫描结果（会话级共享 + 三档徽标上卡片） |
| `1374717` | Merge PR #312（hy/library-external-origin，**#286 切片 3b-1**：卡片「外部来源」短标） |
| `8e66bd8` | feat(mods) 卡片状态 pill 附「外部来源」短标（3b-1 之二） |
| `e8c7fc3` | feat(library) 投影与 DTO 暴露外部导入来源（3b-1 之一，投影 schema 1→2 自动重建） |
| `caae6d9` | Merge PR #311（docs/handoff-286-slice3a，上一版 HANDOFF） |
| `ded781f` | docs HANDOFF 更新到 8cc2cc6 |
| `8cc2cc6` | Merge PR #310（hy/external-mod-state-ui，**#286 切片 3a**：详情弹窗按需扫描） |
| `ef22277` | feat(mods)「无法判定」补原因提示（真机验收时维护者被 unknown 态困惑） |
| `afa7e22` | test(mods) 徽标三档文案与错误映射（6 条新增，3 组控制组见红） |
| `3b8a8ae` | feat(mods) 详情弹窗接线「游戏目录状态」区块（切片 3a 之二） |
| `e01186c` | feat(mods) 外部状态扫描的前端数据层（切片 3a 之一） |
| `479c9bd` | Merge PR #308（hy/external-mod-state-commands，**#286 切片 2b 第 3 步**） |
| `3564b14` | docs(app) `TaskKind` 注释措辞修正（开 PR 前自审） |
| `747f788` | test(runtime) 覆盖任务服务与查询语义（8 条新增，4 组控制组全部见红） |
| `40011c9` | feat(tauri) `start_external_mod_state_scan` + `get_external_mod_state`（含契约登记） |
| `10b7bc6` | feat(runtime) 外部 MOD 状态扫描任务服务（新增 `TaskKind::ExternalStateScan`） |
| `7f305dd` | Merge PR #307（hy/external-mod-state-scanner，**#286 切片 2b 第 2 步**） |
| `823c0a4` | docs(runtime) 缓存键说明补上 `game_id`（开 PR 前自审） |
| `7dc36fa` | Merge PR #306（docs/handoff-2b-scanner） |
| `ed758d5` | test(runtime) 覆盖外部 MOD 状态结果存储（13 条新增，全文件 26 条） |
| `bd903ac` | feat(runtime) 外部 MOD 状态结果存储（`ExternalStateScanCache`，切片 2b 第 2 步） |
| `de9142c` | docs HANDOFF 更新到 4dc1951 + 记录 #286 切片 2b 进度 |
| `097bb34` | Merge PR #303（hy/external-mod-state-scanner，**#286 切片 2b 第 1 步**） |
| `559de43` | feat(runtime) 新增 `ConfiguredExternalStateScanner`（三段式加锁 + 挂 `HmmRuntime`） |
| `eafee73` | refactor(app) 把外部状态扫描拆成 prepare 与 summarize 两段 |
| `26d0b07` | feat(ports) 新增只读的 `InstallGameFileInspector` |
| `4dc1951` | Merge PR #302（docs/handoff-refresh，上一版 HANDOFF 更新） |
| `012b9a7` | Merge PR #301（hy/external-mod-state-scan，**#286 切片 1 + 2a**） |
| `ff08a9a` / `f44ffe3` | docs HANDOFF 注明切片 1/2a 已合并、更新到 a9ead2b 并订正一处错误论断 |
| `02250c6` | feat(app) 外部 MOD 状态扫描服务：有界并发、可取消、只读（**#286** 切片 2a） |
| `b78cb8e` | refactor(view) #301 自查后修两处 API 瑕疵（开 PR 前自审） |
| `25b001a` | feat(core+view) 外部 MOD 状态判定的纯逻辑，无 IO、无写入（**#286** 切片 1） |
| `a9ead2b` | Merge PR #299（docs/verify-parity，本地校验跑 CI 同款策略 + 对等矩阵） |
| `026436f` | fix(tools) 让 `verify.ps1` 再跑一遍 `check-policy.mjs`，并用矩阵锁住两边 |
| `5833bc3` | Merge PR #297（hy/about-update-version-check，**#288**） |
| `fbc7bad` | fix(about) 复查失败时不再让旧结论冒充本次结果 |
| `2aeb317` | fix+docs 修正契约文档链接，并记录本地校验漏掉 CI 检查的坑 |
| `ec24a1c` | feat(about) 「检查更新」真的查询最新版本（**#288**） |
| `912d9a3` | Merge PR #296（docs/troubleshooting-pitfalls） |
| `150bcdc` | docs 实测排障手册里 4 条未验证的说法并订正 |
| `8c5452f` | docs 自审排障手册，修正 4 处事实错误 |
| `24d997f` | docs 排障手册补 6 条「症状指不到原因」的坑 |
| `2bba598` | Merge PR #295（hy/install-failure-phase-copy） |
| `62470d7` | test 安装失败文案改用完整清单校验，堵住「key 被删无人发现」 |
| `77fc33f` | feat(i18n) 安装失败的 4 类 phase 补上可操作的三语文案 |
| `3f753af` | Merge PR #294（hy/install-ambiguous-content-root-phase） |
| `de7acd5` | docs+test 补齐契约的 phase 枚举，并加守卫测试 |
| `9d7269c` | fix(install) 合集包走直接安装时不再退化成「无法生成安装计划」（**#284 R5**） |
| `f4a7fcc` | Merge PR #293（docs/handoff-head-3b6c80d） |
| `c82a17f` | docs HANDOFF 更新到 3b6c80d |
| `3b6c80d` | Merge PR #291（fix/wrapper-directory，**#284**） |
| `d75c7de` | test(import) 补内容根测试缺口（嵌套 nativePC 取最浅 / 根级+包装内并存），并记录大小写变体问题 |
| `121414a` | fix(install) 合集包单独报错，不再混成「无法读取导入文件」（#284 R1） |
| `d12701f` | fix(import) 解析导入包的「内容根」，支持 `nativePC` 外套包装目录（**#284**） |
| `954e063` | Merge PR #290（fix/empty-install-plan，**#285**） |
| `e1b3b35` | fix(install) 采纳 PR #290 评审意见——取消优先、revision 语义不变（#285） |
| `c506e6f` | fix(install) 空计划必须失败，不能报成「安装成功」（**#285**） |
| `e81f5f6` | Merge PR #289（docs/sponsor-in-readme） |
| `bf41cb8` / `2e58a42` | docs README 赞助渠道展示 |
| `0dcb202` / `e040a7e` | docs 重写交接文档到 1af44ab |
| `1af44ab` | chore(deps) Cargo.lock 撤掉 hmm-runtime 的 tracing 依赖 |
| `8851a8d` | fix(runtime) 准入拒绝日志改用安全 app 日志通道（#273 补） |
| `ef87ac9` | fix(runtime) 沙箱模式下 GUI 安装不再被写入准入结构性拒绝（**#273**） |
| `57f32f9` | docs 排障手册补「边界不变量断言的方向陷阱」（#283） |
| `3af7bf2` | fix(mods) 预览图平移：渲染尺寸 clamp + 拖动重锚（**#283**） |
| `0362fbb` / `8ffce8a` | docs 排障手册补坑 |
| `4c4d3c2` | fix(mods) 批量替换目标名称改为渲染时按语言投影（**#282**） |
| `f17ebf1` | fix(install) 常规安装不得静默抢占其他 MOD 的安装目标（**#278**） |
| `56d3070` | fix(install) 卸载时回收残留的替换绑定（#278 方向 a3） |

Issue 状态：

| # | 状态 | 说明 |
|---|---|---|
| 272, 276, 278, 280, 281 | CLOSED | — |
| **282** | **CLOSED** | 根因不是 `useCallback` 漏依赖，而是**名称在载入时就解析成字符串存进 state**（违反 I18N-08）。修法是回到渲染时投影。当前 `pnpm lint` 全项目 **0 error / 0 warning** |
| **283** | **CLOSED** | 6 项验收全部真机通过。判定「用的是 1024 还是 768」**不能看画质**——1024 是 upscale 出来的（356×768 → 474×1024），要看 URL 的 variant 段 + `naturalWidth` |
| **273** | **CLOSED** | 方案 b：GUI 准入只校验游戏根，app-data 根豁免。已真机验证 + 控制组 + 日志通道三验 |
| **284** | **CLOSED** | 主线在 #291（内容根解析 + 合集包单独报错）；**真机验收又抓出 R5**，已由 #294/#295 修完。遗留 R4 已单独开 #292 |
| **285** | **CLOSED** | PR #290 合并：空计划在 commit 前拦截为 `install_failed:empty_plan` |
| **288** | **CLOSED** | 「检查更新」真的查询最新版本（PR #297）。不下载/不校验/不写文件，网络请求留在 Rust 侧，**CSP 与 capability 一行未改** |
| **292** | OPEN | #284 的 R4，三种修法（放宽匹配 / 归一化 / 只改提示）**等维护者拍板**，未实施。#309 已修（HMM 自己不再制造小写变体），#292 回到「用户手工包的边角案例」，取向可按原三选一重估 |
| **286** | **进行中（收尾）** | 外部 MOD 接管。切片 1 → 9c **全部合并**（PR #301/#303/#307/#308/#310/#312/#313/#314/#315/#317），**adopt 已落地**：PR #320（后端全链路）+ PR #321（前端接线）均已 verify + 真机验收 + 合并。**3b-2 A+**（会话表提到应用级，PR #322）在开、已验收待合并。拍板记录都在 issue 评论：3b-2 方案 A（5512751059）、归因（5513829324 / 5514092714）、adopt 4 条规则（5522790071，规则 3 unreadable = **阻断**）、adopt 设计要点（5522985219）、完成汇报 + A+/哈希门两条拍板（5524997376：A+ 做；哈希门不改只记录）。**剩余**：见第六节第 1 条的后续清单 |
| **298** | OPEN | #288 遗留：启动时检查更新 + 导航提示（要动导航项与引导锚点，维护者决定暂缓） |
| **300** | OPEN | #286 后续：孤儿文件检测（只读报告，不给删除动作） |
| **304** | OPEN | #286 缺口：「写锁即判据」的失效条件——`save_game_instance` 不走 game/profile 写锁。含三种修法与代价，等拍板 |
| **305** | **CLOSED** | PR #319 方案 A：沙箱侧读不到 → 该文件一律 `Unreadable`（与游戏侧对称），`files[]` 与比对集恒等长同序——原实现不只少一个文件，还会让后面每个状态**错位**到前一个文件名上 |
| **309** | **CLOSED** | PR #318 方案 D：物化 ZIP 条目名改用原始段，NFKC + 小写的归一化键只用于碰撞检测与指纹。**存量已物化包不自愈、不迁移**（拍板：从未发布正式版，存量只在开发机）——删了重导（排障手册 3.4） |
| **287** | OPEN | 国内分发渠道——#288 负责「告知有没有新版」，#287 负责「从哪里下」 |
| **275** | OPEN | Mod 存储目录可配置。**依赖 #273，现在可以开工**，但「存储目录必须落在沙箱根内」的校验语义要按新准入模型重新确认（app-data 根已豁免） |
| 274 | OPEN | catalog 武器目标别名，依赖外部数据来源审计 + 签核 |
| 277 | OPEN | vite 冷启动偶发卡死，复现不稳定，诊断型 |

工作区：**干净**。PR #316 至 #321 的功能分支远程已清理；`hy/external-state-session-store`（PR #322）
在开。本地还残留 `hy/external-mod-adopt-backend`、`hy/external-mod-adopt-ui` 等已合并分支与若干历史
`codex/*`、`feature/*` 分支，删前用 `git log main..<b> --no-merges` 确认为空。
本文件这次的改动落在本地分支 `docs/handoff-286-adopt`。

## 五、沙箱模式与验收（#273 落地后的新常识）

- 沙箱根取**游戏根的父目录**最省事，不用复制游戏目录、不用改游戏配置：

```
沙箱根   = %APPDATA%\dev.helsincy.modmanager\game   ← 非空，需手动放 marker
游戏根   = ...\game\mhw-minimal                      ← 在沙箱内 ✓
app-data = %APPDATA%\dev.helsincy.modmanager         ← 沙箱外 → 豁免
```

- marker 内容必须**字节精确**为 `{"kind":"hmm.sandbox","schemaVersion":1}\n`（41 字节，
  普通文件非空链接）。marker 只在首次获取 capability 时创建，非空目录无 marker →
  `MarkerRequired`。
- **判断当前沙箱根靠反推**：游戏目录（`config/games.json` 的 `root_dir`）没变 +
  却出现 `sandbox_write_root_rejected` ⇒ 沙箱根不含游戏根。
  配合 `logs/app/` 里 `application.started` 的时间戳可确认是否重启过。
  **别指望读进程环境变量**：`Get-Process hmm-tauri` 的 `StartInfo.EnvironmentVariables`
  **读得到、不报错**（实测返回 63 个变量），但那份变量**不是目标进程的环境**
  （不含 `HMM_SANDBOX_DATA_DIR`）。旧版本这两份文档都写成「会报『无法对 Null 数组进行
  索引』」——**这是错的**，已订正。它的危险在于读得到却给错答案：报错你会换方法，
  错数据你会据此下结论。
- **日志只证明「没被拒绝」，文件系统才证明「真写进去了」**——验收要两处都对上。
- **跑完控制组记得把沙箱根换回来**。控制组（沙箱根不含游戏根，例如 `D:\HMM-sandbox`）
  跑完若不复原，之后每次安装都会被 `write_safety_rejected` 拒掉，看起来像新 bug。
  判断当前沙箱根：**只能靠 marker + 时间戳反推**——别指望进程环境变量。
  `StartInfo.EnvironmentVariables` 能读、不报错，但返回的是一份**与目标进程无关**的
  变量表（实测：读得到 63 个变量，但不含 `HMM_SANDBOX_DATA_DIR`）。
  具体做法：看哪个目录里的 `.hmm-sandbox.json` 的 mtime 与最后一次启动吻合
  （配合 `logs/app/` 的 `application.started` 时间戳确认是否重启过）。
- **改完代码必须重编再验收**：`target/debug/hmm-tauri.exe` 是 cargo 产物，
  不重编就重启，测的还是旧行为（`pnpm tauri:dev` 会自动触发 cargo build）。

## 六、待办（挑一个开工，别一次铺开）

1. **#286 外部 MOD 接管**（**主线已完成，剩收尾**）。已定的口径：按需扫描（不做进
   每次翻页）、扫描做成**后台任务 + 有界并发**（`std::thread::scope`，worker 数
   `min(4, available_parallelism)`，**不引 rayon**）、徽标**三档降级**（tech 完整 /
   classic·grid 精简 / list 极简）、「外部来源」放 pill 内、接管后允许重装、
   孤儿文件另开（#300）、CLI 延后。
   切片顺序：①纯逻辑 → ②扫描服务（PR #301）→ ③扫描器装配（PR #303）→
   ④结果存储（PR #307）→ ⑤command 接线 + 契约（PR #308）→ ⑥3a 详情弹窗按需扫描
   （PR #310）→ ⑦3b-1 卡片「外部来源」短标（PR #312，provenance 走投影新列
   `external_import_adapter_id`，schema 1→2 自动重建）→ ⑧3b-2 卡片三档徽标
   （PR #313，**方案 A 会话级共享**：弹窗每拿到 getter 结果就上报页级 Map，
   翻页保留、路由切换消失、换配置档清空，零后端改动）→ ⑨9a 归因后端（PR #314）
   ＋ 9b 弹窗展示归因（PR #315）→ ⑩9c 卡片徽标全占用改口（PR #317）→ ⑪**adopt**：
   PR #320 后端（core `adopted` 标记 → app 纯判定 → runtime 接管器 + 任务服务 → tauri 命令 +
   契约/LOGGING/SECURITY 登记）+ PR #321 前端（按钮按可用性亮灭、alertdialog 二次确认、
   三语稳定码文案、完成后先刷库再置 installed）——**以上全部已合并**。
   ⑫**3b-2 A+**（PR #322，待合并）：会话表从页级 Map 提到应用级 `ExternalStateSessionProvider`，
   按 (game, profile) 作用域记账，路由切换不丢、切配置档整表换新。
   **adopt 已落地的不变量（不要重造）**：接管 = **只写安装清单不碰文件**；可认领集 =
   `matched` ∧ 无主（清单任一条目引用即占用，fail-closed 口径）；changed / missing 只计数；
   任一 unreadable **阻断**（拍板，非强确认）；空集拒绝；该 MOD 已有条目拒绝（走重装）；
   清单 status 非 TrustEntries 拒绝。写事务：锁外前置拒绝（零副作用）→ 跨进程准入 →
   **阻塞**写锁（写操作不像扫描那样 try_lock）→ 写入准入（与 install 同一条 lifecycle 链）→
   锁内双重验（stat 指纹 vs 扫描记录；以当下清单重算认领集并与用户确认的预览比对，任一漂移
   `external_mod_adopt_stale`）→ 提交屏障 → 原子 `save_manifest`（**带投影追踪的装饰仓储**，
   库列表才会刷成已安装）→ 审计 `adopt_external_mod`（成功写审计失败 → completed 事件带
   `external_mod_adopt_audit_unavailable`，不改写成功事实）。成功后**丢弃**该 MOD 的扫描记录。
   条目形状与 GUI 安装同形：`backup_ref` 空、`installed_file` 必填、`revision_id` 空、
   `adopted: true`（条目级标记，重装/替换整体换条目时自动消失）。前端 hook 里扫描与接管
   **互斥**（接管消费的正是那份记录），completed **先重查再回调**（后端已丢记录，重查让
   会话表不再把这只 MOD 说成「外部已安装」）。
   **后续（按优先级）**：
   - ~~卸载确认弹窗对接管条目补提示~~ → 收尾①已在分支 `hy/adopt-uninstall-warning` 落地（见下）；
     顺带纠正了「重装后卸载可还原」这条错误建议。CLI `install uninstall` 预览
     （`UninstallPlanSnapshot`）还没带 `adoptedFileCount`，与 GUI 对齐是一行映射 + 一行人读输出，
     留给 CLI/批量 adopt 那一片一起做；
   - 契约里 `external_state_scan_*` 仍只登记通配族（adopt 已逐个登记）、`install_audit_unavailable`
     未在契约出现——可顺手补；`ExternalStateSessionProvider` / `externalStateSession.ts` 已在
     i18n 无硬编码中文清单（PR #322）；
   - CLI / 批量 adopt；#304（写锁判据的失效条件）；#300 孤儿文件。
   **验收时确认过、拍板不改的既有行为**：接管后手改文件再卸载会被哈希门挡住
   （`install_uninstall_failed:uninstall`，恢复扫描报「目标变更」，一个文件不删），把文件复原后
   卸载即可（真机验过）——与正常安装被手改后一致，对没有备份的接管条目尤其正确（排障手册 3.5）。
   **「接管 → 重装 → 卸载」不是还原原版的路径**（读代码纠正，不再写「理论可行」）：重装沿用旧条目
   的 `backup_ref`（接管为空 → 仍为空），单修订版重装预览被 `candidate_already_installed` 挡；
   两处确认文案已删掉这条建议，只留 Steam 验证。收尾①（分支 `hy/adopt-uninstall-warning`）：
   `InstallRecoverySummary` / `InstallManifestStatusSummary` 增 `adopted_file_count`（后者
   `Option`，投影派生为 `None`，DTO 省略键），卸载确认弹窗在接管数 > 0 时加「接管文件」指标与
   三语提示，并纳入漂移比对。
   **9a/9b 已落地的归因事实（不要重造）**：占用是**正交事实**，哈希四态判定一个不动；
   归因在三段式 stage 3 与指纹复核**同一锁窗口**读清单（写锁在手 = 无安装在改清单）；
   清单读失败以 `external_state_scan_manifest_unavailable` 整体失败（fail-closed，
   静默少报占用会复刻「外部已安装」误导）；清单不存在 = 全无占用，正常路径。
   归属视图 `first_manifest_entry_by_target`（hmm-app install.rs）**与
   `cross_mod_target_conflicts` 是两个查询，不能合并实现**——冲突判定对畸形态
   （同路径多条异主条目）取「任一被踩即冲突」，合并会弱化安装门禁
   （排障手册 4.9，原设计写「复用」已在 issue 评论修正）。占用者显示名 getter 时按
   `get_mod_detail` 取名链解析（analysis 取名 + 用户改名覆盖），结果只存 mod_id、
   MOD 已删回退显示 id；DTO：文件级 `claimedByModId/claimedByModName`（None 省略键）、
   汇总级恒在的去重 `occupiedBy`。
   **adopt 的前置四项（2026-09-02 评论）全部关闭**：第三层归因（9a/9b/9c）、#305（PR #319）、
   #309（PR #318）、4 条认领规则（评论 5522790071 拍板）。接线约束仍然成立：**结果不能进进度事件**
   ——契约禁止 payload 携带 `target_path`，扫描结果落存储 + 单独 getter；接管没有独立 getter，
   事件只带 `resultRef = modId`，成功即等于用户确认的预览。

   **已落地部分的关键不变量**：`ConfiguredExternalStateScanner`（PR #303）
   挂在 `HmmRuntime.external_state_scanner`。三段式加锁——锁内只 stat、锁外才 hash；
   拿不到准入或有写入进行中 → `Stale`（降级，非失败）；期间文件被改 → **丢弃结果**。
   `try_lock` 而非 `lock()` 是刻意的：用 `lock()` 时「有写入进行中 → Stale」这个分支
   永不执行（实测：退化后该用例不是变红而是**永久挂起**）。
   stat 走新增的 port `InstallGameFileInspector`（`hmm-ports/src/install.rs`），
   不是 `read_game_file`。结果落 `ExternalStateScanCache`（进程内、不持久化、上限 512 条，
   淘汰按 `computedAt` 且时间戳并列时按 key tie-break——不 tie-break 会「单独跑过、
   整组跑挂」）；`query()` 刻意**不拿锁**，因为看到并发修改正是这次 stat 的目的，
   撕裂只会多报 stale、方向 fail-closed。
   已知缺口：**#304**（写锁判据的失效条件）仍 OPEN；#305 已由 PR #319 关闭。

   **动手前先读 issue 评论**：里面有一张已核实的代码事实表（哪几处可复用、哪几处是坑）。

   adopt 复用而非新造的两条既有能力（已按此落地）：卸载的「无 `backup_ref` → 删除文件」语义
   （接管条目就是无 backup_ref 条目，卸载代码零改动）；`hmm_core::installed_file_summary`
   产出的 `InstalledFileSummary { size_bytes, sha256 }`（接管条目的 `installed_file` 来自扫描那次
   读取，接管在写锁内只 stat 不读文件）。

2. **#292（#284 的 R4）大小写变体的 `nativePC` 装不上**——已开 issue，三种修法与代价都写在里面，
   **等维护者拍板取向再动代码**。#309 修掉了 HMM 自己制造变体的路径，#292 回到用户手工包的
   边角案例。倾向 C（只把报错说清楚，不动匹配规则）：放宽匹配会让清单同时
   出现 `nativePC/x` 与 `NATIVEpc/x`，在 NTFS 上指向同一文件，冲突检测会失效；归一化又要改清单
   事实口径，牵动 #278 的占用判定。带修法进 #292 时，一并补「安装侧大小写混合端到端」用例
   （现在补会固化现状，故 #291 未补）。
3. **发版前置：全量 catalog Sandbox Gate 复验**——`docs/PROJECT_TASK_STATUS.md` 明确写着
   「发版前置仅剩全量 catalog Sandbox Gate 复验」：WR-04 Gate D 的证据基于已退役的
   developer seed，全量 catalog 入库后尚未复验。**需要 disposable Windows Sandbox 环境，
   先确认有没有。**
4. **#275 Mod 存储目录可配置**。开工前先跟用户确认：app-data 根既已豁免，
   「存储目录必须落在沙箱根内」的 containment 该怎么算。
5. **#298 启动时检查更新 + 导航提示**（#288 遗留）——要动导航项与新手引导锚点
   `about.release`，维护者决定暂缓，等明确要动引导时再开。
6. **#300 孤儿文件检测**（#286 后续）——有了扫描能力后只剩一层薄封装，但「如何界定
   游戏原版文件、不把原版全算成孤儿」这个噪声问题还没解，先不急。
7. **#274 catalog 武器目标别名**——依赖外部数据来源审计，先确认数据源。
8. **#277 vite 冷启动卡死**——诊断型，复现不稳定，除非频繁打扰否则优先级最低。
9. 可选的展示优化（非阻塞）：卡片缩略图 `object-fit: cover` +
   `object-position: center top` 是**设计意图**（出自 `41d38a0` / #57），
   代价是 356×768 这种竖图只显示顶部约 56%。若要改进，DTO 已带 `width`/`height`
   而前端没消费，做「按图片比例自适应」成本最低，不必动后端。

## 七、行为纪律

- 一切用户可见文案走 copy 字典三语（zh_cn/en/ja，`satisfies LocaleDictionary` 锁定）；
  后端只出稳定错误码；组件不得硬编码中文。
- 门禁事实一律来自后端，前端只做投影与按码取词，不复算状态。fail-closed 不可破。
- 高风险区（安装清单、路径校验、存储删除、并发）改动必须带测试。
- 破坏性操作必须二次确认：弹窗走 `shared/feedback` 的 `Dialog`，
  `role="alertdialog"` + `closeOnBackdrop={false}` + `initialFocusRef` 指向取消按钮。
- 用户重视：后端安全优先、中文交流、小步提交、**不接受把没跑过的测试说成已通过**。
- **没测过的事实不要写进文档**。本轮我凭「跑了 A 脚本」就外推「B 脚本也不含某检查」，
  写进排障手册并据此向维护者提了错误建议——实测 B 脚本是好的。**测了 A 不等于知道 B。**
- **提交前跑 `scripts/verify.ps1`（完整入口），不要用单个 `check-*.ps1` 代替**：
  `check-policy.ps1` 只查文件/脚本的存在性与大小写，内容检查（链接/边界/密钥/体积）
  由 verify.ps1 里的 7 个脚本 + `check-policy.mjs` 完成（见 `docs/TROUBLESHOOTING.md` 2.4）。
  交接文档里写「已完成」的项，动手前先扫一眼实体是否真的存在
  （上一手就有 `DeleteConfirmationDialog` 只写了 import、组件本体没有的情况）。
- **改行为后新增的回归用例必须跑控制组**：把修复退回去，**逐条**确认新用例会变红。
  没红的就是假绿（恒真断言）。2026-08-30 靠这条抓出过自己写的假绿用例
  （「图永不被拖出视口」只断言了右边缘，而往右拖时离开视口的是左边缘）。
  **不要因为「别的红了」就判定整组有效。**
- **控制组本身也可能有假绿——「我以为在测这个」是最常见的失败模式**。本轮 5 组控制组
  里有 **2 组第一次没测到点上**：一组改错了分支（改了 `map_scan_error`，而用例守的是
  更早的取消检查入口），一组的改法被被测代码一并拒绝、等于没改。两次都是用例照常绿，
  但那只是**没碰到它在守的东西**。判据是：确认改完的那条路径**确实会被该用例执行到**，
  而不是「代码看起来相关」。控制组没让用例变红时，先怀疑控制组，再下结论。
- **退化成阻塞等待时，用例不是变红而是挂起**。把 `try_lock` 换回 `lock()` 后，目标用例
  跑了 60 秒以上不返回——比变红更危险，因为不一定被 CI 超时捕获。跑控制组时用有界
  `timeout` 包住，并把「挂起」也算作预期的失败信号。
- **做对照实验必须确认两次运行的外部条件一致**。本轮 verify 曾报 10 条 `hmm-cli`
  sandbox 测试失败，怀疑是本机 `MonsterHunterWorld.exe` 在跑（`TESTING.md` 有记载）；
  在干净 worktree 上重跑却全过，一度以为否定了假设——实际是**重跑发生在游戏退出之后**，
  两者根本不是对照实验。事后 `Get-Process -Id <pid>` + `tasklist` 双重复核才确认。
  **「对照组通过」也可能只是外部条件变了。**
- **评审里「已知但未修」的缺口必须转成 issue，不能只留在 PR 评论里**——
  PR 一旦合并归档，评论就再也不会有人看，缺口等于丢失。PR #291 的 R4 就是这样：
  先记在评论里，合并时才补开 #292。开 issue 时把**可选修法各自代价**一并写进去，
  让拍板的人不用重读 PR。
- **关闭 issue 的门槛是验收标准逐条为真，不是「剩下的应该没问题」**。
  验收标准里捆在一起的多个动作（如「双击重置 + Esc + 点背景关闭」）要拆开分别标注，
  #283 就是这么收的尾。

## 八、开工前先做

1. `git log --oneline -5` 和 `git status` 确认工作区干净
2. 读 `.workbuddy-ai/memory/` 最近两天的日志
3. 跟用户确认要挑哪一条待办，不要自己假设
````

---

## 附：真机验收环境（回归用）

- fixture 生成：`HMM_FIXTURE_OUT_DIR=D:/DEV/HMM-WR-fixture cargo test -p hmm-runtime --test generate_weapon_fixture`
- 应用游戏目录已配置：`%APPDATA%\dev.helsincy.modmanager\game\mhw-minimal`（含前置桩）
- 库里有一对 one001 双胞胎（**字节完全相同**，两沙箱 sha256 已核对）：
  `weapon-mod-one001-wrapped` = 沙箱 `mod-import-1787939069837-0`、
  `weapon-mod-one001-flat` = 沙箱 `mod-import-1787939077192-1`。
  **名字 ↔ 沙箱的对应以 `mod-import/results.json` 的 `display_name` 为准**——
  本文件旧版本把两者写反过，据文档直接下结论会指错 MOD。
  2026-09-03（adopt 验收收尾）后**两只都未安装**，游戏根下 one001 路径干净，清单只剩
  `mod-import-1788182264438-0` 一条；
  `nativePC/wp/one/one002/mod/` 下有一组**无主外部文件**（manifest 零引用，
  历次验收遗留的场景素材，别当垃圾清掉）。另有 5 个狩技盒子导入包
  （受 #309 影响扫描恒「无法判定」）
- 缩略图回归包：`tmp/thumb-reclaim-test.zip`。**维护者已决定不纳入 git**（`tmp/` 被
  gitignore），换机器后若丢失照下面重建即可（关键是结构，图本身随便一张竖图都行）：

  ```
  thumb-reclaim-test.zip
  └── thumb-reclaim-test/                          ← zip 根下就是这一层，别压平
      ├── nativePC/common/hmm_thumb_reclaim_test.bin  # 任意内容，只为让包能被识别
      ├── preview.jpg                                 # 356×768 竖图，最贴近真实场景
      └── readme.txt                                  # 说明用途，可省
  ```

  识别规则：扫描器先找 `nativepc` 目录，取其**父目录**作候选根，只收**直接子文件**
  里的图片；`preview.*` 优先级最高，扩展名只认 png/jpg/jpeg/webp。
  （现包内 `readme.txt` 原文：`HMM thumbnail reclaim test package. Used to verify that
  deleting a mod removes sandboxes, thumbnails and catalog entries.`）
  **#284 之前**该包装不上是「正常」的：zip 里套了 `thumb-reclaim-test/` 包装目录，
  解包不剥单层包装目录 → 相对路径根不在 `allowed_install_roots`（MHW 只有
  `["nativePC"]`）→ 全被过滤，审计里是 `action_count: 0` 的空计划。

  **#284（PR #291）之后该包应当能装成功**：导入后点安装 → 任务日志走到
  `commit.processing → completed`、审计 `action_count=1`、游戏根下真的出现
  `nativePC/common/hmm_thumb_reclaim_test.bin`。它是 #284 的标准回归包，
  **装上失败就说明内容根解析退化**。
  包内多个 `nativePC`（合集包）仍应拒绝，且报「包内有多个 nativePC 目录，
  请拆分后分别导入」（不再是「无法读取导入文件」）。
- **判定预览图用哪个变体**：看 URL 的 variant 段 + `naturalWidth`。
  768 → 356×768（65777 B），1024 → 474×1024（98891 B，upscale 出来的）。
  脚本 `tmp/check-preview-1024.mjs`（CDP 连 9223）。
- **触发 1024→768 回落**：把沙箱源图 `preview.jpg` 改名（detail 每次打开时现扫沙箱，
  不读 thumbnails 缓存；卡片 768 来自持久化的 revision catalog 不受影响）。
  改名后**不要**重启/重导/清缓存，但必须**重新打开弹窗**。
- **验证语言切换**（I18N-08）不能走「去设置页切语言」——那条路会卸载页面再重新拉取，
  改不改都是对的。必须 preference 设为**跟随系统** + 触发 `languagechange`
  （CDP `Emulation.setLocaleOverride`）。脚本 `tmp/check-locale-switch.mjs`。
  判定唯一有区分度的是「名称就地变化」。
- **外部状态扫描（#286）的验收姿势**：one001 双胞胎字节相同，**任一**沙箱的
  `.../nativePC/wp/one/one001/mod/`（`one001.mod3` + `.mrl3`）都是外部状态的
  **标准复制源**：复制进夹具游戏根 → 「已安装」；删掉其中一个 → 「部分安装」；
  用 `Add-Content -NoNewline` 追加字节 → 「已被改动」；stale 路径的触发是
  「改动后不重扫、直接重开详情」（getter 重新 stat 发现漂移 → 旧结果 + 过时提示）。
  - **占用归因（9a/9b 已上线）的标准场景**：HMM 里安装其中一只，扫另一只——
    弹窗徽标仍「外部 已安装」（哈希判定不动，设计如此），但下方有**占用提示行**
    （正文色）+ 文件行「已被「X」占用」小胶囊；改名占用者后重开详情，名字**随查随取**；
    卸载占用者并重扫后，占用信息随清单条目一起消失。**卡片**此时仍显示绿色「已安装」
    ——那是 9c 要改口的内容，验收 9c 之前别当 bug 追。
  - 验收清单里**破坏性步骤（卸载/删文件）排到所有依赖该场景的检查之后**，
    多语言核对紧挨场景搭建做——2026-09-03 就因为「先卸载再看英文文案」误报过一次
    （排障手册 4.10）。
  - 狩技盒子导入的那 5 个包是 **#309 修复前**的存量物化包（沙箱内容根 `nativepc`），
    扫描恒「无法判定」，**不会自愈**——要用它们验收先在 HMM 里删掉再从狩技盒子重导。
  - **接管（adopt）的标准脚本**：flat 版沙箱两文件复制进游戏根 → 检查 → 「接管 2 个文件」
    → 确认 → 清单多两条 `adopted: true` / `backup_ref` 空、审计 `adopt_external_mod -> success`
    → wrapped 版重扫显示「已被「flat」占用」→ 卸载删掉两个文件。**负例**：改 mrl3 长度后
    **不重扫**直接接管 → `external_mod_adopt_stale`。注意顺序：改文件必须在点接管**之前**；
    先接管后改文件会撞卸载的哈希门（排障手册 3.5），那是既有保护不是 bug。
  - **验收命令里的 PowerShell 变量（`$game` / `$sb` / `$mf`）是窗口级的**：换一个窗口执行
    `Add-Content "$game\one001.mrl3"` 时 `$game` 为空，路径展开成 `\one001.mrl3` 写到当前盘根目录，
    命令照样成功、游戏目录纹丝不动——2026-09-03 就这样把「接管应被拒」误看成「接管成功了」。
    所有验收命令在同一窗口跑，改完文件先 `(Get-Item ...).Length` 核对再回应用点按钮（排障手册 4.12）。
