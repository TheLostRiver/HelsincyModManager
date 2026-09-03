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
| `pnpm` / `corepack` 报「禁止运行脚本」`PSSecurityException` | Windows 默认策略 Restricted，`.ps1` 全局 shim 全被拦 | [1.6](#16-powershell-禁止运行脚本pnpm-等全局-shim-报-pssecurityexception) |
| 往 `tmp/` 放个脚本，`eslint .` 就报一堆 `no-undef` | gitignore 不影响 eslint 扫描范围 | [2.1](#21-tmp-下的脚本让-eslint-报-no-undef) |
| 新写的 scripts 测试从没被执行 | `pnpm test` 的 glob 只覆盖 `src/**` | [2.2](#22-scripts-测试没被执行) |
| 缩略图生成成功但界面不显示，协议请求无响应 | WebView2 不支持非标准 scheme | [3.1](#31-自定义协议请求无响应) |
| `git push` 卡住不动，或报 `could not read Username` | WSL 的 git 没有凭据，在等交互式输入 | [1.7](#17-git-push-卡住不动并报-could-not-read-username) |
| `gh run watch --exit-status` 退出码是 0，但 CI 其实还没跑完 | `$?` 拿到的是管道里 `tail` 的退出码 | [1.8](#18-gh-run-watch-退出码是-0-但-ci-还在跑) |
| 只想给文档改一句话，diff 却变成整行重写 | 编辑未逐字符匹配，全角标点被半角化 | [1.9](#19-只改一句话-diff-却变成整行重写) |
| 跑了 `check-policy.ps1` 说 passed，CI 却在链接/边界/密钥上红 | 它只查存在性与大小写，内容检查要跑完整 `verify.ps1` | [2.4](#24-只跑单个-check-ps1却以为策略检查过了) |
| 真机验收时装不上，或刚合入的修复「没生效」 | 沙箱根还停在控制组 / 二进制没重编 | [3.3](#33-真机验收时装不上或修复没生效) |
| 删掉某个文案 key 后测试照样全绿 | 防退化断言用的是数量下限，永不失败 | [4.6](#46-防退化断言永不失败) |
| 玩家看到兜底文案，但前端后端测试都全绿 | 错误码与前端文案 key 不对应，缺 key 静默回落 | [4.7](#47-后端错误码与前端文案-key-不对应) |
| 切语言后界面文案不更新，且 lint 有一条长期被忽略的 warning | 展示名在**载入时**就解析成字符串存进 state，违反 I18N-08 | [4.3](#43-语言相关的展示数据必须在渲染时投影) |
| `pnpm lint` 长期挂一条 warning，verify 却一直是绿的 | `pnpm lint` 没有 `--max-warnings 0` | [4.4](#44-warning-不阻塞-verifywarning-不等于无害) |
| 图片放大后拖到边缘就回不来，只能重置 | clamp 用了视口尺寸而非渲染尺寸，且拖动未重锚（死区） | [4.5](#45-边界不变量要断言方向无关的量) |
| 新加的回归用例在把修复退回去后**依然绿** | 不变量断言写成了单侧、方向选反——假绿 | [4.5](#45-边界不变量要断言方向无关的量) |
| 代码里打了日志，dev 终端能看到，`logs/app/app-*.log` 里却没有 | 普通 tracing 不过 app 日志层；字段白名单外会被整条拒绝 | [6.1](#61-加了-tracingwarn-但-logsappapp-log-里什么都没有) |
| `node --test` 报 `ERR_MODULE_NOT_FOUND`，但被导入的 `.ts` 文件明明存在 | node 直载 .ts 不做无扩展名补全；type-only 导入被擦除所以「别处这样写没事」 | [4.8](#48-node---test-直载-ts值导入必须带-ts-扩展名) |
| 重构后既有测试全绿，但某个安全门禁在边角输入下悄悄变松 | 「同一份数据的两个查询」被合并成一个实现，语义差异只在畸形态出现 | [4.9](#49-同数据的两个查询不要合并实现) |
| 按验收步骤走到第 N 步，功能「没出现」，疑似 bug | 前面某步销毁了该功能依赖的场景，不是缺陷 | [4.10](#410-验收步骤会自毁场景顺序即前置) |
| 狩技盒子导入的包装不上（空计划）、扫描恒「无法判定」，zip 导入的同一包却正常 | #309 修复前物化把内容根写成 `nativepc`，存量包不自愈，删了重导 | [3.4](#34-狩技盒子导入的包装不上扫描恒无法判定309-修复前的存量包) |
| 断言路径大小写的测试一直绿，bug 修不修都绿 | NTFS 大小写不敏感，`join(..).exists()` 恒真；要枚举目录项逐字比对 | [4.11](#411-断言路径大小写不能靠-joinexistsntfs-上它恒真) |

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

### 1.6 PowerShell 禁止运行脚本：`pnpm` 等全局 shim 报 PSSecurityException

**症状**（在 PowerShell 里执行 `pnpm tauri:dev`）：

```
pnpm : 无法加载文件 D:\DEV\HappyCode\npm-global\pnpm.ps1，因为在此系统上禁止运行脚本。
FullyQualifiedErrorId : UnauthorizedAccess
```

**根因**：`Get-ExecutionPolicy -List` 各 scope 全是 `Undefined`，Windows 客户端默认
等效 `Restricted`，一切 `.ps1` 都跑不了。npm/pnpm 在 Windows 上的全局 shim 默认是
`.ps1`，所以 `pnpm`、`corepack` 这类命令在 PowerShell 里直接被打回——
**这不是仓库或依赖的问题**。

**处理**（任选一）：

1. **改当前用户策略，一劳永逸**（不需要管理员，`CurrentUser` 写在 HKCU）：

   ```powershell
   Set-ExecutionPolicy -Scope CurrentUser RemoteSigned
   ```

   之后 `pnpm tauri:dev`、`scripts\verify.ps1` 都恢复正常。
2. **不改策略**：显式调用 `.cmd` 入口——它走 cmd.exe，不经 PowerShell 脚本引擎：

   ```powershell
   pnpm.cmd tauri:dev
   ```

**不要在 Git Bash 里跑 `pnpm tauri:dev` 绕开它。** 那里的 `pnpm` 是 sh shim，
确实不受 PowerShell 策略影响，但 `tauri.conf.json` 的 `beforeDevCommand` 是
`corepack pnpm dev`，而 Git Bash 下 corepack 会拼出 `d:\c\Users\...` 错乱路径
（见 1.4），vite 起不来。

`corepack.cmd pnpm --version` 在本机验证可用，走 cmd 的调用链是通的。

### 1.7 git push 卡住不动并报 could not read Username

**症状**：

```
fatal: could not read Username for 'https://github.com': terminal prompts disabled
```

或者更糟——命令**卡在那里不动**，既不成功也不失败。

**根因**：WSL 里的 git **没有配置凭据**，HTTPS 推送于是在等待交互式输入用户名——
在非交互终端里表现为卡死。可复核：

```bash
git config --get credential.helper                              # WSL：空
powershell.exe -NoProfile -Command "git config --get credential.helper"
# Windows：!"D:/Tools/PortableGit/mingw64/bin/git-credential-manager.exe"
```

两侧结果不同，正是同一台机器上「Windows git 能推、WSL git 不能推」的原因。

**处理**：

- 推送走 **Windows 的 git**：

  ```powershell
  powershell.exe -NoProfile -Command "git push -u origin <branch>"
  ```
- 想让它在没凭据时**快速失败**而不是卡住，设 `GIT_TERMINAL_PROMPT=0`。
- 只读操作（`git fetch`、`git ls-remote`）不需要凭据，可以继续用 WSL 的 git——
  所以「推送失败了吗」要用 `git ls-remote` 复核，**不要靠重试判断**（网络层报错
  不代表服务端没收到，可能已写完才断开）。
- 想在 WSL 里直接用 `gh`，调 Windows 版：
  `"/mnt/c/Program Files/GitHub CLI/gh.exe"`。`WindowsApps\gh.exe` 不要碰：它是
  App Execution Alias，不是普通文件——WSL 的目录列表里看得见，但访问与执行都报
  `No such file or directory`（PowerShell 里 `Test-Path` 同样返回 False）。

### 1.8 gh run watch 退出码是 0 但 CI 还在跑

**症状**：`gh run watch <id> --exit-status` 明明还在刷新（in_progress），
后面的 `echo $?` 却打印 `0`，于是误判 CI 已绿。

**根因**：`$?` 是管道**最后一条命令**的退出码。写成
`gh run watch <id> --exit-status | tail -5; echo $?` 时，拿到的是 `tail` 的退出码（恒 0），
gh 的退出码被管道吃掉了。

**处理**：

```bash
# 要退出码就别接管道
gh run watch <id> --exit-status; echo $?

# 确实要管道的话
gh run watch <id> --exit-status | tail -5; echo "${PIPESTATUS[0]}"
```

**另外一个独立的坑**：PR 自己的 run 与**合并后 main 上的 push run 是两个不同的 run**。
PR 全绿不等于合并后 main 全绿——收尾要等后者：

```bash
gh run list --branch main --limit 1 --json databaseId
```

别用 `sleep` 轮询，用上面的 `gh run watch <id> --exit-status`。

**还有一个坑**：`gh run watch` 要等到 CI 跑完，而这套 CI 实测 6.5~8.7 分钟——**可能超过
单次命令的时长上限被中止**。别把「命令被中止」误读成「CI 卡住」：用有界等待，中止后用
`gh run view` 查真实状态，不要再开一个 watch。

```bash
timeout 230 gh run watch <id> --exit-status      # 有界等待
gh run view <id> --json status,conclusion        # 查状态用这条
```

### 1.9 只改一句话 diff 却变成整行重写

**症状**：只想给某个超长文档条目追加一句话，`git diff` 却显示**整行被重写**，
动辄上千字的改动量，看不出你真正改了什么。

**根因**：编辑工具的待匹配文本与原文**不是逐字符一致**（典型是把全角 `：` 写成了
半角 `:`），工具仍报告替换成功，但实际按自己的规范化重写了整行——把行内所有
全角 `，`/`：`/（）`/`——` 都换成了半角。`FRONTEND_BACKEND_CONTRACT.md` 里有
近 1900 字符的长条目（实测最长行 1869 字符、全文件 2 行超过 1000 字符），中招代价很大。

**处理**：

1. 中文长行一律用脚本做**精确替换 + 计数断言**：

   ```python
   assert source.count(old) == 1, source.count(old)
   source = source.replace(old, new)
   ```
2. 改完核对「只有预期变化」：用 `difflib.SequenceMatcher` 打印 opcode，
   或统计全角标点数量是否**只随新增句子增长**。
3. 看到 diff 异常大就立刻 `git checkout -- <file>` 还原重做，**不要在坏掉的基础上
   继续改**。

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

### 2.4 只跑单个 `check-*.ps1`，却以为「策略检查过了」

**症状**：跑了 `scripts/check-policy.ps1`，输出 `Policy check passed.`，
于是以为策略检查通过；推送后 CI 却在 **Markdown 链接 / 前端边界 / 密钥扫描 /
文件体积 / 空白**这些内容检查上失败，而本地复现不出来。

**根因**：`check-policy.ps1` 是一个**单检查脚本**，只做 `policy.json` 里的
**存在性与大小写**检查（`requiredFiles` / `caseSensitiveFiles` / `requiredScripts`，
外加根目录必须是 `AGENTS.md`）——**不做任何内容检查**。

完整策略是一**组**脚本。`scripts/verify.ps1` 会依次执行全部 7 个：

```
check-policy.ps1              存在性 / 大小写
check-whitespace.ps1          行尾空白
check-file-size.ps1           文件体积
check-forbidden-files.ps1     禁用文件
check-doc-links.ps1           Markdown 链接
check-frontend-boundaries.ps1 前端分层边界
check-secrets.ps1             密钥扫描
```

**处理**：提交前跑完整的 `scripts/verify.ps1`，**不要用单个 `check-*.ps1` 代替**。

#### 两套实现：本地是 PowerShell，CI 是 Node

CI 跑在 Linux，只能执行 `node scripts/check-policy.mjs --scope verify`；
Windows 本地跑的是上面那组 PowerShell 检查器。**两者是独立实现**——
今天对同一个问题给出同样的结论（实测：同一个坏链接两边报同样的错、退出码都是 1），
但没有任何机制天然保证以后不漂移。

因此 `verify.ps1` 在跑完 ps1 检查器后**再跑一遍 CI 用的那个 mjs**，
保证「本地过了」等价于「CI 会过」。`scripts/verify-entrypoints.test.mjs` 另有一个
**检查矩阵**把两边钉住：新增策略检查必须**两边都登记**，否则用例红。

#### Markdown 链接的相对路径基准是「文件所在目录」

从 `docs/FRONTEND_BACKEND_CONTRACT.md` 链到 `docs/release/UPDATER_PLAN.md`
要写 `release/UPDATER_PLAN.md`；写成 `../release/UPDATER_PLAN.md` 会被解析成
**仓库根**的 `release/`，从而报链接无效。同理，在 `docs/` 下互相引用不要加 `../`。

#### 一个不自动跑的检查

`scripts/check-governance-changes.ps1` **不在任何自动入口里**：它只打印
「治理文件有变更，建议人工 review」，**不阻断**（退出码始终 0）。
真正的强制 review 由 `.github/CODEOWNERS` 保证（覆盖 `AGENTS.md`、`policy/**`、
`scripts/**`、`.codex/**`、`.agents/**` 等）。需要本地提醒时手工跑：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-governance-changes.ps1 -Mode working
```

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

### 3.3 真机验收时装不上或修复没生效

**症状**（两种，都容易误判成代码问题）：

1. **任何安装都失败**，errorCode 为 `install_failed:write_safety_rejected`
   （重装任务则是 `install_reinstall_failed:write_safety_rejected`）。
   注意**普通安装没有 preflight 阶段**：它会在 `install.plan.building` 之后直接
   `install.failed`——`install.commit.processing` 只在准入通过的分支里发，所以被拒时
   日志里根本看不到它。`install.reinstall.preflight.processing` 是重装任务自己的阶段。
2. **刚合入的修复在真机上「没生效」**，行为与合并前一样。

**根因**：

1. 沙箱根还停在**控制组**配置（一个不含游戏根的目录，如 `D:\HMM-sandbox`）——
   上次跑控制组验收后忘了换回来。方案 b 只校验游戏根，游戏根不在沙箱根内就会被拒，
   **这是正确行为，不是 bug**。
2. `target/debug/hmm-tauri.exe` 是 cargo 产物。不重编就重启，测的是旧行为
   （#284 验收时该产物 mtime 为 08-30 21:41，而合并提交是 08-31 16:55，差约 19 小时）。

**处理**：

- 实验组沙箱根取**游戏根的父目录**，游戏根在内、app-data 在外，marker 已存在：

  ```powershell
  $env:HMM_SANDBOX_DATA_DIR = "$env:APPDATA\dev.helsincy.modmanager\game"
  pnpm.cmd tauri:dev
  ```
- **判断当前沙箱根只能反推**。别试 `Get-Process hmm-tauri` 的
  `StartInfo.EnvironmentVariables`：实测它返回 63 个变量且**不含** `HMM_SANDBOX_DATA_DIR`
  ——.NET 只对由自己启动的进程填充 `StartInfo`，拿它判断沙箱根会得出错误结论。
  改用两条证据：看哪个目录里的 `.hmm-sandbox.json` 的 mtime 与最后一次启动吻合，
  配合 `logs/app/app-*.log` 里 `application.started` 的时间戳确认是否重启过。
- 验收前先看二进制新鲜度：`target/debug/hmm-tauri.exe` 的 mtime 应**不早于**最后一个提交。
- **跑完控制组必须把沙箱根换回来**，否则之后每次安装都被拒，看起来像新 bug。

**顺带一条验收方法**：同一个用户动作的**多个入口要逐个点**。合集包在「预览安装」
给出正确文案，但「右键菜单 → 直接安装」走的是另一条链路——#284 的 R5 就是这个，
只点一个入口会漏。

### 3.4 狩技盒子导入的包装不上、扫描恒「无法判定」（#309 修复前的存量包）

**症状**：经「外部导入」（狩技盒子）进库的 MOD，安装报「包内没有找到可安装的文件」
（#285 的 `empty_plan`），#286 的「检查游戏目录」恒为「外部 · 无法判定」；而同一个
MOD 用 zip 导入就一切正常。

**根因**：不是包的问题，是 **#309 修复前的 HMM 自己**——旧版物化管线把 NFKC + 小写
的归一化键拿去当内部 ZIP 条目名，沙箱内容根落成 `nativepc`，而 MHW 适配器的
`allowed_install_roots = ["nativePC"]` 大小写敏感，整包被过滤成空计划、比对集为空。
看沙箱即可确认：`%APPDATA%\dev.helsincy.modmanager\mod-import\sandboxes\external-import-package-*\`
下是 `nativepc` 就是存量包。

**处理**：修复后**新导入的包**内容根保留原始大小写；**已物化的存量包不会自愈**，
也不做迁移（拍板：从未发布过正式版本，存量只在开发机上）——在 HMM 里删除该 MOD，
再从狩技盒子重新导入一次即可。手工改沙箱目录名不在支持范围内：沙箱是 HMM 的存储，
不做手工维护（revision catalog 本身不记录文件路径，所以问题不在「会不会不一致」，
而在于不该把手工改存储当成处置手段）。

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

### 4.3 语言相关的展示数据必须在渲染时投影

**反例（#282）**：批量生命周期的替换目标名在**载入事实时**就调用
`resolveReplacementTargetNames(displayNames, locale)` 解析成字符串，存进 workflow
state。后果是切语言后已经打开的下拉不会更新——要更新只能重新拉取事实，而那会丢掉
用户已选的目标。

这类缺陷很容易被误诊成 `useCallback` 依赖数组漏了 `locale`。补依赖确实治好
「切语言后**再进入**流程」，但治不好「**已在流程里**切语言」，因为病灶是
**解析时机**，不是闭包新鲜度。**先问一句这份数据在 state 里是原始值还是已投影值**，
再决定改哪儿。

**契约（I18N-08，见 `src/features/replacements/replacementTargetNames.ts` 头部）**：
DTO 携带全语言 `displayNames`，展示名在**渲染时**按 fallback 链投影，语言切换不重拉
列表。参照实现是 `ReplacementTargetPanel`。

### 4.4 warning 不阻塞 verify，warning 不等于无害

`pnpm lint` 没有 `--max-warnings 0`，`verify.ps1` 不会因为 warning 变红。
#282 那条 `React Hook useCallback has a missing dependency: 'locale'` 因此安静地
待了很久，被当成无害噪音，实际是个真 bug。

**处理**：看到 warning 先判断它是「风格建议」还是「可能的行为缺陷」。
`react-hooks/exhaustive-deps` 属于后者——它报的正是闭包会捕获陈旧值。
做不到零 warning 时，至少记住哪几条是已知的、为什么留着。

### 4.5 边界不变量要断言「方向无关的量」

**反例（#283）**：给预览图平移写的「图永不被拖出视口」用例，断言的是图像**右**边缘
是否还在视口内。可是往右拖的时候，离开视口的是**左**边缘——右边缘只会越拖越远。结果
这条用例在**有 bug 的实现下照样绿**，是恒真的假绿。

抓到它的是控制组验证：把修复退回去重跑，另外两条新用例如期变红，**这条没红**。

**处理**：

1. 断言「不能越界」这类不变量时，挑一个**与方向无关的量**——重叠宽度、覆盖率、
   绝对值距离——而不是某一侧的边缘坐标。#283 改成断言「图像与视口的重叠宽度 ==
   理论最大覆盖」之后，旧实现下立刻变红。
2. 确实要断言单侧的话，把**两个方向都遍历**（往左拖到底、往右拖到底）。
3. 无论如何都要跑控制组：把修复逐处退回去，确认**每一条**新用例都变红。没红的那
   些就是假绿——不要因为「别的红了」就判定整组有效。

#### #283 的两个真实缺陷（供对照）

- **`object-fit: contain` 下 clamp 平移范围必须用渲染尺寸，不是视口尺寸**。留白轴的
  渲染尺寸远小于视口：竖图在宽视口里高度受限，2x 时横向根本没有溢出，按视口算出的
  范围却允许横拖一整个视口宽度，图直接被推出屏幕。
- **拖动要在每个指针采样后重锚到已提交的 offset**。锚在按下时的指针位置、累积原始
  位移的话，被 clamp 截掉的越界量不会消失，而是累积成**死区**：拖过界后指针得把整个
  越界量原路走回去，图才会重新动——用户看到的就是「拖到空白处然后拖不动了」。

两条叠在一起才有了「拖出屏幕 + 拖不回来」的完整症状，只修任一条都还是坏的。

### 4.6 防退化断言永不失败

**症状**：写了「防退化」断言，控制组也跑了，但**删掉某个 key / 分支后测试照样全绿**，
护栏等于没设。

**根因**：断言用的是**数量下限**这类几乎不可能失败的条件，例如：

```js
assert.ok(Object.keys(dict).length >= 8);
```

真实风险是「某个**具体**的 key 消失」，而总数从 12 掉到 11 仍然满足 `>= 8`。
（本次排查：12 个文案 key 里有 5 个**没有任何用例断言**。其中 `empty_plan` 是实测的——
删掉它之后 564 条测试零失败，而它正是 #285 专门为「装了但没装上」写的。另外 4 个
`lock` / `complete` / `recovery_pending` / `recovery_unavailable` 是 grep 确认无断言引用，
**未逐个实测**，属同等风险。）

**处理**：

1. 验**集合**而不是数量：

   ```js
   assert.deepEqual(Object.keys(dict).sort(), expected.sort());
   ```

   少了 key、多了未登记的 key，都会红。
2. 写之前先问一句：**这条断言可能失败吗？** 想不出让它红的输入，它就是装饰。
3. 判别方法与控制组同：真的把目标 key 删掉跑一次，看它红不红。
4. **「顺带被别的用例把守」不等于有把守**：某个 key 被抓到，可能只是因为别的用例
   恰好硬编码了它（本次 12 个里只有 7 个是这种「偶然覆盖」）。覆盖要显式。

### 4.7 后端错误码与前端文案 key 不对应

**症状**：后端新增了稳定的错误码 / phase，后端单测全绿、前端测试全绿、CI 全绿，
但**玩家看到的是兜底文案**（如「安装失败」），完全不知道发生了什么。

**根因**：前端按 `install_failed:<phase>` 去掉前缀去查 `installFailures[phase]`，
**缺 key 时用 `?? default` 静默回落**。而测试是分层的：后端测「错误码对不对」，
前端测「三语 key 集合是否一致」——**没人测「phase 与 key 是否成对」**，
于是两侧各自全绿。

**处理**：

1. 新增 phase 必须同步补三语文案（`FRONTEND_BACKEND_CONTRACT.md` 已写明），
   并在用例里登记进**完整必需清单**（见 4.6）。
2. 空串要单独判：`??` 只在 nullish 时回落，`""` 不会被兜底文案接住。
3. **验证只能靠真机点一遍**：合成该错误码的输入，看界面文案。
   这一层是单测替代不了的——#284 的 R5 就是真机才发现的。
4. 相关：**契约里 phase 的枚举会悄悄过期**。phase 有三个来源，只 grep 调用点的
   字符串字面量会漏掉 `error.code()` 与 `error.failure_phase()` 这两族**非字面量**
   来源（#284 R5 时漏了 4 个 `write_admission_*`）。

### 4.8 node --test 直载 .ts：值导入必须带 .ts 扩展名

**症状**：`node --test` 加载某个 `.ts` 模块时报
`ERR_MODULE_NOT_FOUND: Cannot find module '...externalInstallStatusView'`，
但那个文件明明存在，且 vite / tsc 下一切正常。

**根因**：node 的 type stripping 只擦类型，**模块解析不做无扩展名补全**。
更迷惑的是：同目录别的模块「同样写法却没事」——因为那些是 `import type`，
运行时被整个擦掉，根本不参与解析。只有**值导入**才踩雷。

**处理**：可被 `node --test` 直接或间接加载的 `.ts` 模块，相对**值导入**一律带
`.ts` 扩展名（仓库已开 `allowImportingTsExtensions`，先例：
`useBatchModLifecycleWorkflow.ts` 的导入、`externalCardBadge.ts`）。
type-only 导入可以不带（会被擦除）。相关：桶文件值导入的限制见仓库硬约束。

### 4.9 「同数据的两个查询」不要合并实现

**症状**：把两处「看起来重复」的判定重构成共享助手，既有测试全绿——
但某个安全门禁在**畸形态输入**下悄悄变松了。

**案例**（#286 第三层归因）：`cross_mod_target_conflicts`（安装冲突门禁）与
占用归因都查「清单里这条路径归谁」。但对**同一路径多条异主条目**（畸形态）：
门禁的语义是「**任一**条目被计划踩到即冲突」（fail-closed），归因的语义是
「归属取**首条**」。合并成「先取首条再判冲突」后，门禁在「计划的 MOD 拥有首条、
另一 MOD 还有一条」时**放行了原本要拦的写入**——而现有测试没有畸形态用例，
全绿依旧。

**处理**：

1. 动手前，把两个调用方在**退化/畸形态输入**下的期望各写一行；不一致就不共享实现，
   各自保留并在两处注释互引说明为什么不能合并。
2. 控制组要用**畸形态输入**打：正常输入下两种实现恒等价，控制组必然假绿。
3. 「唯一判定出处」是好原则，但它指**同一语义**只实现一次——不是把不同语义
   硬塞进同一实现。

### 4.10 验收步骤会自毁场景：顺序即前置

**症状**：按验收清单走到第 N 步，要验的界面元素「没出现」，被当成 bug 上报；
排查半天发现功能没问题。

**根因**：清单里某个**靠前**的步骤销毁了后续步骤依赖的场景。案例（9b 验收）：
第 ④ 步「卸载占用者，确认占用提示消失」把占用场景拆了，第 ⑤ 步「切英文看占用
文案」自然什么都看不到——占用者都没了，文案本就不该出现。

**处理**：

1. 写验收清单时，每一步先问：**它销毁了什么状态？后面还有谁依赖这个状态？**
   破坏性步骤（卸载/删除/清理）排到所有依赖该场景的检查之后。
2. 多语言、多视图这类「同场景换个投影」的检查，紧挨着场景搭建步骤做完，
   再进入破坏性步骤。
3. 收到「步骤 N 没看到 X」的报告，先核对步骤 N-1 之前有没有场景销毁步骤，
   再去怀疑代码。

### 4.11 断言路径大小写不能靠 `join(..).exists()`：NTFS 上它恒真

**症状**：测试写着「沙箱里应有 `nativepc/fixture.bin`」并且一直绿，夹具明明写的是
`nativePC`；修掉大小写 bug 后这条测试在本机照样绿——它从来没测到大小写。

**根因（#309）**：Windows 的 NTFS 默认大小写不敏感，`Path::join("nativePC").is_file()`
对磁盘上的 `nativepc` 同样返回 true（本机实测：`existsSync("nativePC/fixture.bin")`
对 `nativepc/` 目录为 true）。于是同一条断言在两个平台上是两种东西：Windows 上
**恒真**，Linux CI 上则把**小写化 bug 固化成了预期行为**——`materializer_builds_a_sandboxed_package…`
就曾断言 `nativepc`，修复时若只把字面量改成 `nativePC`，在 Windows 上依然假绿。

**处理**：

1. 断言「大小写是否保留」只能**枚举目录项、逐字比对名字**（`fs::read_dir` →
   `file_name()`），例如 `exact_child_names(&dir) == ["nativePC"]`。
2. 跑控制组时把 bug 放回去（条目名 `to_lowercase()`），确认断言的**失败信息里出现
   两种拼写**（`left: ["nativepc"]` / `right: ["nativePC"]`），而不是只看红绿。
3. 同理，NTFS 上造不出 `A.bin` 与 `a.bin` 并存的夹具；要验证「大小写不敏感碰撞仍被拒」，
   用 NFKC 等价段（如全角字母 `ｎativePC`）制造碰撞。

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

## 6. 日志与诊断

### 6.1 加了 `tracing::warn!` 但 `logs/app/app-*.log` 里什么都没有

**症状**：代码里明明打了日志，dev 终端能看到，`logs/app/app-YYYY-MM-DD.log` 里却搜不到，
而且没有任何报错——事件像被吞了一样。

**根因**：app 日志层（`hmm-infra/src/app_log.rs`）只处理 **target == `hmm.safe_app_log`**
的事件，`on_event` 第一行不匹配就 return。**普通 `tracing::warn!` / `info!` 一律留在
文件层之外**——该文件自己的注释就写着 "ordinary tracing stays outside the file layer"。

更隐蔽的是第二条：字段有**白名单**，未知字段会把整条事件判为 `invalid` 并拒绝
（`into_validated_record` → `EventRejected`）。所以即便补上 target，只要带了一个白名单
外的字段（比如自定义的 `stage`），这条日志照样不落盘；而且从日志本身完全看不出来，
只会在 `/diagnostics` 的 health 里表现为事件被拒。

**处理**：走 `emit_safe_app_log(AppLogEvent::warning(event_name).with_xxx())`。
字段名只能用白名单内的这些：

```
event_name  task_id  game_id  profile_id  mod_id  task_kind  task_status
phase  operation  result  error_code  safe_path  item_count  duration_ms
```

自定义语义要**映射到既有字段**：比如「准入阶段」放 `operation`、「子步骤」放 `phase`。
事件名与 code 类字段受 `validate_stable_code` 约束：只允许小写字母、数字与 `_.-`，
且不得包含敏感文本（路径、用户名等一律不行）。

**校验方法**：别只看编译通过。要么在测试里用 `tracing::subscriber::with_default` +
scoped 日志层断言落盘内容，要么真机触发一次**失败**路径后去看日志文件——
成功路径不会留下这行日志，验证不了通道是否通。
