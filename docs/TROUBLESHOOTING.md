# 开发环境与工具链排障（TROUBLESHOOTING）

> 状态（2026-08-30）：初版，收录的都是**报错信息指不到真正原因**的问题——按报错字面
> 去查会浪费大量时间。除「自定义协议」一节外都属于环境与工具使用方式问题，不是仓库
> 缺陷。范围：本机 Windows 开发环境、Tauri/WebView2 运行时行为、校验脚本（verify.ps1）
> 与测试基建的坑。不涉及产品功能设计与安全边界，那部分见各自的设计文档。

## 症状速查

| 症状 | 真正的原因 | 跳转 |
| --- | --- | --- |
| `failed to open: target/debug/.cargo-lock`（os error 5），所有 cargo 都卡住 | `target/` 里留了删不掉的 0 字节僵尸文件 | [1.1](#11-cargo-卡在-cargo-lock) |
| `hmm-infra --lib` 成片失败，集中在 symlink / 目录删除相关测试 | 同上，不是代码问题 | [1.2](#12-hmm-infra-测试成片失败) |
| 清理 dev 链后另一个项目 / IDE 会话异常 | 无差别 `Stop-Process -Name node` 误杀 | [1.3](#13-清理-dev-链时误杀其他进程) |
| `corepack` 报 `d:\c\Users\...` 这类错乱路径 | Git Bash 下 corepack shim 路径解析 | [1.4](#14-git-bash-下-corepack-路径错误) |
| `git commit -m` 多行中文消息报 `unexpected EOF` | Git Bash 引号解析，与消息内容有关 | [1.5](#15-git-commit-多行消息报-unexpected-eof) |
| 往 `tmp/` 放个脚本，`eslint .` 就报一堆 `no-undef` | gitignore 不影响 eslint 扫描范围 | [2.1](#21-tmp-下的脚本让-eslint-报-no-undef) |
| 新写的 scripts 测试从没被执行 | `pnpm test` 的 glob 只覆盖 `src/**` | [2.2](#22-scripts-测试没被执行) |
| 缩略图生成成功但界面不显示，协议请求无响应 | WebView2 不支持非标准 scheme | [3.1](#31-自定义协议请求无响应) |

## 1. 环境与工具链

### 1.1 cargo 卡在 .cargo-lock

**症状**：任何 cargo 命令都卡在

```
failed to open: target/debug/.cargo-lock  (os error 5)
```

**根因**：在会话（AI agent / 自动化工具）里用后台方式跑 cargo 时，**后台命令会被强制
沙箱化**（`dangerouslyDisableSandbox` 对后台无效）。实测同一个 `cargo check -p hmm-core`
前台成功、后台失败。沙箱化的 cargo 会在 `target/` 里留下 **0 字节僵尸文件**，
rename / delete 一律 EPERM，之后所有 cargo 都拿不到锁。

**处理**：目录级改名不会被文件级僵尸挡住，同卷改名瞬时：

```powershell
mv target/debug target/debug-poisoned
mkdir target/debug
# 把 build deps examples incremental .fingerprint 搬回新目录
```

**规避**：长耗时 cargo **不要在会话里跑**。工具通常还有单条命令 10 分钟上限，
而 `nohup ... & disown` 的进程会随工具调用结束被杀。交给人在本地终端跑：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

### 1.2 hmm-infra 测试成片失败

**症状**：`cargo test --workspace` 报若干 `hmm-infra --lib` 失败，集中在
`install_commit.rs`、`reinstall.rs` 的 symlink 与目录删除相关用例。

**根因**：**环境问题，不是代码问题**。见 1.1——僵尸文件破坏了文件系统语义，
依赖 symlink 与目录删除的测试首当其冲。

**判断方法**（不靠重编对照，重编一次太贵）：

1. 失败的源文件是不是都在 `git status` 里？不在 = 你没动过它们。
2. 你的改动是不是纯新增？纯新增不会改变既有用例的前置条件。
3. 这些文件最后一次改动，是否早于上一个已知绿色的 commit？是 = 干净环境下它是过的。

三条都成立就别改代码，先怀疑环境。本仓库已两次 verify 全绿证伪过这条
（`hmm-infra` 均 415 passed / 0 failed）。

### 1.3 清理 dev 链时误杀其他进程

**症状**：跑完清理脚本后，`E:\DEV\bolo-pi` 之类的其他项目、或 IDE 会话本身出问题。

**根因**：本机 `node.exe` 进程里长期混着三类东西——WorkBuddy 自身进程
（`~/.workbuddy-ai/binaries/node/`）、别的项目（如 bolo-pi）、当前项目的 dev 链。
`Get-Process node | Stop-Process -Force` 会一起杀掉。

**处理**：一律按端口精确定位，并且先确认端口是否真的在监听（很多时候 dev 链已自己
退干净，根本不用杀）：

```powershell
foreach ($p in 1420, 9223) {
  $c = Get-NetTCPConnection -LocalPort $p -State Listen -ErrorAction SilentlyContinue
  if ($c) { $c | ForEach-Object { Stop-Process -Id $_.OwningProcess -Force } }
}
Get-Process -Name hmm-tauri -ErrorAction SilentlyContinue | Stop-Process -Force
```

拿不准归属时用命令行分辨：

```powershell
Get-CimInstance Win32_Process -Filter "Name='node.exe'" |
  Select-Object ProcessId, CommandLine
```

### 1.4 Git Bash 下 corepack 路径错误

**症状**：corepack 拼出 `d:\c\Users\...` 这类错乱路径。

**根因**：Git Bash 下 corepack shim 的路径解析有问题。

**处理**：直接调用 corepack 的 JS 入口：

```powershell
"D:/Nodejs/node.exe" "D:/Nodejs/node_modules/corepack/dist/corepack.js" pnpm <script>
```

`verify.ps1` 内部走 `cmd /c corepack pnpm`，不受影响。

### 1.5 git commit 多行消息报 unexpected EOF

**症状**：

```
/usr/bin/bash: eval: line 1: unexpected EOF while looking for matching `'`
```

即使整个消息用单引号包裹仍会触发，与消息里的中文标点和反引号组合有关。

**处理**：改用文件传消息：

```powershell
git commit -F tmp/commit-message.txt
```

同理，`gh` 写长文（issue 正文/评论）一律用 `--body-file`，不要用 `-b`；
`gh -l "a, b"` 会把空格带进标签名，要写成 `-l a -l b`。

## 2. 前端与校验

### 2.1 tmp/ 下的脚本让 eslint 报 no-undef

**症状**：往 `tmp/` 放个诊断脚本后，`pnpm lint` 报一堆
`'fetch' is not defined` / `'console' is not defined` / `'WebSocket' is not defined`。

**根因**：`tmp/` 被 gitignore，但 **eslint 不会因为 gitignore 而跳过目录**，它有自己的
忽略配置。

**处理**：`eslint.config.js` 的 global ignores 里已包含 `tmp`（与 `armor-data` 同性质：
不纳入版本管理的本地状态）。放临时脚本前先确认它还在。

顺带：`pnpm lint` **没有** `--max-warnings 0`，warning 不会让 verify 失败。

### 2.2 scripts 测试没被执行

**症状**：新写了 `scripts/xxx.test.mjs`，但 verify 里从没见它跑。

**根因**：`package.json` 的 test 脚本是

```
node --test "src/**/*.test.mjs"
```

**只覆盖 `src/`**。`scripts/` 下的测试必须在 `scripts/verify.ps1` 里逐条显式登记：

```powershell
Write-Host "Running xxx tests..."
node --test scripts/xxx.test.mjs
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
```

本仓库曾有三份测试（check-policy 15 条、prepare-windows-sidecars 14 条、
windows-installer-cleanup-config 7 条）因此休眠，现已在 verify.ps1 登记。

### 2.3 vite 冷启动白屏

冷启动可能白屏 1–2 分钟（issue #277）。等 vite 进程 CPU 平息后 Ctrl+R，
不要反复重启。端口 1420 已保留，devUrl 与 vite 都钉死 `127.0.0.1`，不要改回
`localhost`。

## 3. Tauri / WebView2

### 3.1 自定义协议请求无响应

**症状**：后端缩略图生成成功、catalog 记了、DTO 带了、CSP 放行了、协议也注册了，
但界面不显示。CDP 里能看到 `Network.requestWillBeSent`，之后**没有任何响应事件**。

**根因**：**Windows WebView2 不支持非标准 scheme**。wry 用 workaround 绕过
（`custom_protocol_workaround` 模块），由此产生两个连锁后果：

1. 过滤器只注册 `http://<scheme>.*`，**只匹配 http(s)**。所以 `thumbnail://` 请求
   根本进不了处理器，报 `ERR_UNKNOWN_URL_SCHEME`。
2. `http://thumbnail.localhost/X` 能命中，但 wry 命中后会把它还原成
   `thumbnail://localhost/X` —— **多出一个 `localhost/` 段**。处理器若不剥掉它，
   就会把 `localhost` 当成 package_id，一律返回 400。

**处理**：

- 持久化层保持平台无关的 `thumbnail://<pkg>/<variant>/<hash>`；
- 只在 DTO 出口按平台改写成 webview 能加载的形态（Windows 用
  `http://thumbnail.localhost/...`）；
- 处理器同时接受三种形态：custom protocol、Windows localhost origin、
  wry 还原出的 `thumbnail://localhost/...`。

**不要**用 `http://thumbnail.<pkg>/...` 这种把 package_id 塞进 host 的写法——它实测
能返回 200，但 CSP 的 host-source 不支持后缀通配，采用它会逼着把 `img-src` 放松到
`http://*`。

相关实现见 `src-tauri/src/thumbnail_protocol.rs`，契约见
`docs/FRONTEND_BACKEND_CONTRACT.md` 的「Mod 预览图」部分。

### 3.2 诊断方法：CDP 探测 + 控制组

这类问题**不需要插日志重编**。dev 链开调试端口后直接探：

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9223"
corepack pnpm tauri:dev
```

然后用 Node 全局 WebSocket 连 `http://127.0.0.1:9223/json/list`，
在 `Runtime.evaluate` 里跑页面内探针，配合 `Network.enable` 收
`requestWillBeSent` / `responseReceived` / `loadingFailed`。

**关键是必须带一个控制组**：请求一个不存在的 package，它**必然**返回 400。
有响应事件 = 处理器被调用了；没有任何事件 = 请求压根没进处理器。
这两者对应完全不同的修复方向，靠猜会浪费一整轮重编。

本仓库已把这套封装成 `scripts/check-thumbnail-protocol.mjs`：

```powershell
corepack pnpm check:thumbnail-protocol
corepack pnpm check:thumbnail-protocol -- --probe <pkg> <variant> <hash>
```

## 4. 测试方法论

这一节不是环境坑，但同样属于「不写清楚就会反复踩」的类别。

### 4.1 检查类代码必须自带反例

校验脚本自身有缺陷时会「通过」得毫无痕迹——**假绿比没有检查更危险**，它给的是
虚假的安全感。写检查器要配齐四样：

1. **反例成体系**：每个「应通过」用例配一个「应失败」用例（孤儿目录、悬挂引用、
   目标仍在……），证明检查器真能看见问题，而不是恒真。
2. **fixture 用真实形状**：坏数据在真实场景里长什么样，fixture 就长什么样。例如
   悬挂引用必然是真实格式的 ID——那个包曾经合法存在过。
3. **一条防退化用例**：故意用不认识的格式/值，要求照样检出。这条最关键——它防的是
   实现退化成「依赖具体格式」（见 4.2）：那种实现在真实格式的用例上照样绿，
   只有非格式用例能戳穿。
4. **真实数据回归**：构造数据过了不算，拿真实 app data 再跑一次。

同一原则的另一个应用：**任何「删除/清理后检查」的脚本，都要先拿清理前的状态
空跑一次**并确认它报 FAIL。否则脚本本身写错时，删完也是全绿，检查等于没做。
`scripts/check-storage-reclaim.mjs` 就是这么验证的。

### 4.2 不要用正则解析结构化文档

**反例**：曾经用正则从 manifest 的整个 JSON 里刮 ID，只认两种已知格式。将来新增
ID 格式的那天，它不会报错也不会警告，只是**静默地什么都看不见**——所有依赖它的
检查一并变成假绿。

**处理**：结构化数据走结构化读取（`entries[].mod_id` / `entries[].revision_id` /
`replacement_bindings[].binding.mod_id`），正则只用于真正没有结构的文本。

## 5. 仓库硬约束

改动前先确认这些隐性检查，它们的报错信息同样指不到原因：

- `src/features/mods/modsLibraryData.ts` 是**生成文件**。要改内容先改
  `scripts/generate-mod-library-mock-data.mjs`，再
  `node scripts/generate-mod-library-mock-data.mjs --count 72`，否则
  `modLibraryMockGenerator.test.mjs` 会红。
- **新增 Tauri command 必须登记** `docs/FRONTEND_BACKEND_CONTRACT.md`，
  `src/shared/api/tauriContractCoverage.test.mjs` 会扫 `generate_handler!` 逐个比对。
- **i18n「无硬编码中文」清单**包含 `ModLibraryPage.tsx`、`ModLifecycleFeedback.tsx`、
  `ModContextMenu.tsx`、`CompactActionPanel.tsx` 等。其 `stripComments`
  **只过滤整行 `//` 注释**——行尾注释里的中文照样被抓。这些文件一律英文注释。
- copy 字典新增 key 要三语齐备，并登记进 `src/shared/i18n/i18n.test.mjs` 的字典清单。
- `verify.ps1` 用 `git diff --check` 查空白，**文件末尾多一个空行直接红**。
- 既有测试存在**用字面量断言源码形状**的情况（如
  `batchSelectionActive && actionId === "preview-plan"`）。重构时保留原表达式形态，
  否则测试红得莫名其妙。
