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

当前检查：

- 统一空白检查。
- 必需文件和大小写检查
- 文件大小硬性限制
- 禁止文件检查
- Markdown 内链检查
- 敏感信息扫描

检查作用域由 `policy/project-policy.json` 的 `checkScopes` 定义。`preCommit` 可以配置局部排除路径，例如 `.codex/**`；`verify` 默认保持全量检查。新增排除目录时优先修改 policy，不应在 hook 或检查脚本里硬编码路径。

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

该工作流在 push 和 pull request 时运行 `scripts/verify.ps1`。

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
- `docs/TESTING.md`
- `docs/release/`
- `policy/`
- `scripts/`
- `.githooks/`
- `.github/workflows/`
- `.codex/`
- `.agents/`

CODEOWNERS 本身不会阻止合并，必须配合 GitHub branch protection / ruleset 中的 review 规则。

## 文件大小强制策略

文件大小硬性限制由 `policy/project-policy.json` 控制。

当前重点防止：

- 单个 `.rs` 文件膨胀到数千行。
- 单个 `.ts` / `.tsx` / `.js` / `.jsx` 文件膨胀到数千行。
- 前端样式、页面、配置、脚本文件长期堆积。
- Markdown 文档无边界增长。

超过硬性限制会导致：

- 本地 `verify.ps1` 失败。
- pre-commit 失败。
- pre-push 失败。
- GitHub Actions `Verify` 失败。

如果确实是生成代码、协议定义、静态 catalog，或与主应用无关的独立工具目录，应加入 allowlist 或 `fileSize.excludePathPatterns`，并在 PR 中解释原因。

当前 `.codex/` 是独立的上下文管理工具，不属于主应用运行时代码边界；文件大小硬性限制默认通过 `fileSize.excludePathPatterns` 排除该目录，避免工具自身演进被主应用代码体量门禁误拦截。

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
- `scripts/verify.ps1`
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
