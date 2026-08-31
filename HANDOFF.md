# HelsincyModManager 会话交接提示词

新开会话时，把下面「提示词正文」整段粘进去即可。文件本身留在仓库根目录方便更新，
`HANDOFF.md` 需要提交就提交，不需要就删掉。

> 本文件最后更新：2026-08-31 24:00，HEAD = `a9ead2b`。
> **可信度用 `git log` 交叉验证**：对比文档里出现的 commit 短号与实际 HEAD 的差集。
> 2026-08-30 曾发现本文件漏了 8 个提交、且 #278 状态写反（写「待做」实际已做完），
> 照旧文档接手会往错误方向做。

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
  **别把「切侧边栏丢多选」当 bug 修。**

## 四、当前进度（2026-08-31 24:00，HEAD = `a9ead2b`）

最近提交（由新到旧）：

| commit | 说明 |
|---|---|
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
| **292** | OPEN | #284 的 R4，三种修法（放宽匹配 / 归一化 / 只改提示）**等维护者拍板**，未实施 |
| **286** | **进行中** | 外部 MOD 接管。**切片 1（纯逻辑）已开 PR #301**；后续切片：IO+后台任务、三语文案与接线、接管 adopt。设计决定与已核实事实都在 issue 评论里 |
| **298** | OPEN | #288 遗留：启动时检查更新 + 导航提示（要动导航项与引导锚点，维护者决定暂缓） |
| **300** | OPEN | #286 后续：孤儿文件检测（只读报告，不给删除动作） |
| **287** | OPEN | 国内分发渠道——#288 负责「告知有没有新版」，#287 负责「从哪里下」 |
| **275** | OPEN | Mod 存储目录可配置。**依赖 #273，现在可以开工**，但「存储目录必须落在沙箱根内」的校验语义要按新准入模型重新确认（app-data 根已豁免） |
| 274 | OPEN | catalog 武器目标别名，依赖外部数据来源审计 + 签核 |
| 277 | OPEN | vite 冷启动偶发卡死，复现不稳定，诊断型 |

工作区：**干净**。PR #289 至 #299 的分支本地与远程均已删除；`hy/external-mod-state-scan`（#286 切片 1，PR #301）为当前唯一开放分支。

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

1. **#286 外部 MOD 接管**（**当前在做**）。已定的口径：按需扫描（不做进每次翻页）、
   扫描做成**后台任务 + 有界并发**（`std::thread::scope`，worker 数 `min(4, available_parallelism)`，
   **不引 rayon**）、徽标**三档降级**（tech 完整 / classic·grid 精简 / list 极简）、
   「外部来源」放 pill 内、接管后允许重装、孤儿文件另开（#300）、CLI 延后。
   切片顺序：①纯逻辑（PR #301，已开）→ ②IO + 后台任务 → ③三语文案与接线 → ④接管 adopt。
   **动手前先读 issue 评论**：里面有一张已核实的代码事实表（哪几处可复用、哪几处是坑）。

   两条最关键的既有能力，**不要新造**：
   - 卸载已有「无 `backup_ref` → 删除文件」语义 ⇒ **接管只写无 backup_ref 的清单条目，
     不用改卸载代码**；
   - `InstalledFileSummary { size_bytes, sha256 }` 已存在（`hmm-core`）⇒ 接管直接复用。

2. **#292（#284 的 R4）大小写变体的 `nativePC` 装不上**——已开 issue，三种修法与代价都写在里面，
   **等维护者拍板取向再动代码**。倾向 C（只把报错说清楚，不动匹配规则）：放宽匹配会让清单同时
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
- 库里现有：wrapped（已安装）、flat（`mod-import-1787939069837-0`，已安装，
  2026-08-30 真机验收时重定向装到了 **one001**）、5 个第三方导入包
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
