# 工程治理与强制约束

本文档说明项目如何把协作规则从“文档建议”升级为“可执行约束”。目标是防止 agent 或贡献者在长期协作中绕过架构边界、堆积超大文件、提交敏感内容或修改治理规则而无人注意。

## 当前已落地的约束层

### 1. 文档层

面向人和 agent 的协作说明：

- `AGENTS.md`
- `.agents/`
- `CONTRIBUTING.md`
- `SECURITY.md`
- `CHANGELOG.md`
- `docs/GOVERNANCE.md`
- `docs/LOGGING.md`
- `docs/TESTING.md`
- `docs/release/`

文档层负责解释“为什么这样做”，但不能单独视为强制约束。

### 2. 机器规则层

机器可读规则位于：

```text
policy/project-policy.json
```

该文件定义：

- 必需文件。
- 大小写敏感文件。
- 必需验证脚本。
- 检查作用域及其路径排除规则。
- 文件大小硬性限制。
- 禁止提交的文件类型和路径。
- 敏感信息扫描模式。
- 治理文件列表。

### 3. 本地验证层

统一入口：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Linux / Steam Deck 开发环境：

```bash
bash scripts/verify.sh
```

当前检查：

- PowerShell/Bash 验证入口契约。
- 统一空白检查。
- 必需文件和大小写检查。
- 文件大小硬性限制。
- 禁止文件检查。
- Markdown 内链检查。
- 敏感信息扫描。
- 前端 typecheck、lint、tests 和 build。
- Rust workspace tests、check 和 `clippy --all-targets -D warnings`。

检查作用域由 `policy/project-policy.json` 的 `checkScopes` 定义。`preCommit` 和 `verify` 默认都检查
受版本管理的 `.codex/skills/hmm*` 项目技能；本地 ignored 的 Codex 工具不属于 Git 候选文件。
新增排除目录时优先修改 policy，不应在 hook 或检查脚本里硬编码路径。

`forbiddenFiles` 另外禁止跟踪 `.codex/**`，只通过窄 allow pattern 放行
`.codex/skills/hmm*/**`。因此即使使用 `git add -f`，非 HMM Codex 文件也会在本地验证和 CI 中失败。

QG-01 将前端 tests 和 workspace clippy 纳入两个统一入口。后续 PR 运行完整统一入口后，不需要再把
这两项作为 CI 缺口额外手工补跑；聚焦测试仍按改动边界执行。当前 Windows 本机观察中，前端 tests
约增加 3-4 秒，clippy 在缓存命中时约增加 20 秒，冷 worktree 或远端 CI 的实际耗时可能更长。

### 4. Git Hooks 层

安装方式：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\install-hooks.ps1
```

安装后：

```text
git config core.hooksPath .githooks
```

当前 hooks：

- `pre-commit`：提交前运行基础检查。
- `pre-push`：推送前运行统一验证，并提示治理文件变更。

Git hooks 可以被绕过，所以它们不是最终门禁。

### 5. CI 层

GitHub Actions 工作流：

```text
.github/workflows/verify.yml
```

该工作流在 push 和 pull request 时通过 `bash scripts/verify.sh` 运行 Linux 原生入口，required job
名称保持 `Policy and docs`。因此 CI 与 Windows 本地入口共享前端 tests 和 workspace clippy 门禁，
不在 workflow 中复制命令。

GitHub CodeQL 使用仓库设置中的 default setup，不由版本库内 workflow 配置。当前分析语言固定为
GitHub Actions、JavaScript / TypeScript 和 Rust。语言选择必须与 PR head 及合并后的默认分支实际受
版本管理的源码保持一致；移除某语言最后一份受跟踪源码时，应在该 PR 的合并门禁前同步取消对应语言，
避免 CodeQL 在没有可分析源码时于 database finalize 阶段失败。新增受支持语言时，应同步更新该设置
和本节，并确认每个已选语言的 analyze job 都达到 terminal `success`。

CI 是当前项目的远程自动门禁。真正强制合并还需要 GitHub 分支保护配合。

### 6. CODEOWNERS 层

治理文件所有权定义：

```text
.github/CODEOWNERS
```

治理文件包括：

- `AGENTS.md`
- `CONTRIBUTING.md`
- `SECURITY.md`
- `CHANGELOG.md`
- `docs/GOVERNANCE.md`
- `docs/LOGGING.md`
- `docs/TESTING.md`
- `docs/release/`
- `policy/`
- `scripts/`
- `.githooks/`
- `.github/workflows/`
- `.github/CODEOWNERS`
- `.codex/`
- `.agents/`

CODEOWNERS 本身不会阻止合并，必须配合 GitHub branch protection / ruleset 中的 review 规则。

## 文件大小治理策略

文件大小软提醒和硬性限制由 `policy/project-policy.json` 控制。

当前重点防止：

- 单个 `.rs` 文件膨胀到数千行。
- 单个 `.ts` / `.tsx` / `.js` / `.jsx` 文件膨胀到数千行。
- 前端样式、页面、配置、脚本文件长期堆积。
- Markdown 文档无边界增长。
- 通过压缩换行把大量代码塞进极少行。

`fileSize.review` 按类别定义非阻断行数提醒；超过时 Node 和 PowerShell 入口输出 review warning，
但保持成功退出。`fileSize.block` 只定义防止灾难性膨胀的行数硬上限。
`fileSize.blockBytes` 对候选文件定义全局字节硬上限，可选的 `fileSize.maxLineLength` 限制受管
文本的单行长度。
现有 `docs/**` 通过 `maxLineLengthExcludePathPatterns` 只豁免单行长度检查，仍受行数和字节上限约束；
lockfile 则必须显式列入 `allowlist`。

review warning 用于发现混合职责和维护性风险，应在相关功能 PR 中拆分或记录暂不拆分的理由，
不能仅为消除 warning 创建独立重构 PR。超过 `fileSize.block`、全局字节或单行长度硬限制才会导致：

- 本地 `verify.ps1` 或 `verify.sh` 失败。
- pre-commit 失败。
- pre-push 失败。
- GitHub Actions `Verify` 失败。

如果确实是生成代码、协议定义、静态 catalog，或与主应用无关的独立工具目录，应加入 allowlist、
`fileSize.excludePathPatterns` 或对应的窄检查排除项，并在 PR 中解释原因。

当前只有 `.codex/skills/hmm*` 属于仓库治理内容，并与其他受管文档一样接受文件大小、空白、链接和
敏感信息检查。其余 `.codex` 内容是本地工具状态，由 `.gitignore` 排除，不通过仓库分发。

## 交付单位与验证分级

默认交付单位是一条可演示的纵向产品能力或一个 release blocker，不是内部 task、文件或文档。一个
纵向 PR 可以包含设计、后端、CLI/Tauri、前端、测试和文档，但应以单一职责 commit 保持边界清晰。
文档同步、测试搬迁、dead-code、文件拆分和内部前置默认并入相邻产品 PR。

只有下列情况才拆 PR：

- 改动彼此无关。
- 需要独立回滚或独立发布。
- 安全风险或玩家数据影响明显扩大。
- diff 已大到无法连贯 review。

本地验证按风险分级：

- Low：docs、隔离内部重构、隔离 UI，运行 touched boundary 的最小检查。
- Medium：跨层行为、public DTO/contract、task/event 语义，运行聚焦测试，并在首次 PR ready 前运行
  一次完整 `verify.ps1`。
- High：安装/卸载/重装、存档、真实文件写入、回滚、安全、并发或治理/CI，运行正负聚焦测试、
  一次完整 `verify.ps1` 和 findings-first 全 diff 自审。

review 小修默认只重跑受影响的聚焦验证。只有风险边界扩大、公共契约或治理规则变化、依赖/基线变化，
或旧完整验证证据已不适用于当前 diff 时，才重复本地完整验证。无论本地分级如何，最终 commit 的
required CI 必须到 terminal `success`；CodeRabbit 缺席时必须独立全 diff 自审，但不因此无限等待或
无条件重复完整本地验证。

## GitHub 分支保护建议

为了让约束真正生效，建议在 GitHub 仓库设置中启用 ruleset 或 branch protection。

建议规则：

```text
Target branch: main
```

启用：

- Require a pull request before merging。
- Require approvals。
- Require review from Code Owners。
- Require status checks to pass。
- Required status check: Verify / Policy and docs。
- Block force pushes。
- Block deletions。
- Do not allow bypassing the above settings。

如果仓库只由一个维护者管理，也建议至少启用：

- Require status checks to pass。
- Require branches to be up to date before merging。
- Restrict direct push to main。

## 治理文件变更规则

修改以下内容时，必须格外谨慎：

- `policy/project-policy.json`
- `scripts/check-*.ps1`
- `scripts/check-*.mjs`
- `scripts/verify.ps1`
- `scripts/verify.sh`
- `.githooks/`
- `.github/workflows/`
- `.github/CODEOWNERS`
- `.codex/`
- `.agents/`
- `AGENTS.md`

这些改动不禁止，但必须被视为“修改规则本身”。PR 描述应说明：

- 为什么需要改规则。
- 是否降低了某项限制。
- 是否影响 agent 行为。
- 是否影响 CI 或 hooks。
- 是否需要同步文档。

## 后续待补强

脚手架落地后继续补：

- Rust workspace 依赖边界检查。
- 前端 ESLint import boundary。
- 包大小和文件数量限制。
- 生成代码 allowlist。
- PR 模板。
- issue 模板。
- release workflow。

## 重要结论

文档只能表达意图，脚本和 CI 才能提供约束。

当前项目已经能拦截典型的超大代码文件、禁止文件、坏链接和明显敏感信息。但要完全防止长期架构腐化，还需要在 Tauri/Rust 脚手架落地后继续用编译依赖、lint 规则和 branch protection 加固。
