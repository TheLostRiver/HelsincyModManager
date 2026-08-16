---
trigger: always_on
---

# 多 Agent 协作手册

> 本文件是 `docs/MULTI_AGENT_COLLABORATION.md` 的 Antigravity / Gemini 规则适配入口。
> 主协作规则以仓库文档为准；修改本文件时应同步检查主文档，避免外部 agent 规则和项目协作文档分叉。

本文档定义 Helsincy Mod Manager 中多个 AI coding agent 协作时的分工、任务包、文件边界、审查门禁和失败处理方式。它面向外部 agent、前端专职 agent、审查 agent 和主控 agent 使用。

目标不是让每个 agent 都读完整个仓库后自由发挥，而是把能力强但上下文不同的 agent 放进受控流程里，让它们各自做擅长的事，同时不破坏项目架构、玩家数据安全边界和前端可维护性。

## 适用范围

本文适用于：

- 由 Codex、Gemini 或其他模型共同完成同一个任务。
- 将 UI 设计、React 组件、CSS、交互动效交给前端专职 agent。
- 让一个 agent 实现、另一个 agent 审查。
- 把大型功能拆成多个互不冲突的子任务。
- 让外部 agent 在不了解完整项目历史的情况下安全参与。

不适用于：

- 真实 Mod 安装、卸载、回滚、存档恢复等高风险逻辑的自由实现。
- 无任务包、无写入范围、无 review 的临时改动。
- 让外部 agent 直接决定项目架构、目录边界或治理规则。

## 核心原则

1. **主控 agent 决定边界，worker agent 只在边界内执行。**
2. **前端 agent 可以负责 UI 手感，但不能负责业务规则。**
3. **任务必须以任务包形式分发，不能只说“照设计稿改一下”。**
4. **每个 worker 必须有明确允许修改文件和禁止修改文件。**
5. **实现完成后必须经过规格审查和架构/代码质量审查。**
6. **未通过 review 的改动不能进入下一个任务。**
7. **验证命令必须真实执行，不能把“应该能过”当成结果。**
8. **所有 agent 都不能回退用户或其他协作者的改动。**

## 角色定义

### 主控 Agent

主控 agent 负责协调，而不是把所有代码都自己写完。

职责：

- 读取 `AGENTS.md`、`docs/ARCHITECTURE.md`、`docs/TESTING.md`、`docs/GOVERNANCE.md` 和本手册。
- 将需求拆成小任务。
- 为 worker agent 编写任务包。
- 决定允许修改文件和禁止修改文件。
- 回答 worker agent 的澄清问题。
- 对 worker 结果做集成。
- 触发规格审查、代码质量审查和最终验证。
- 保证工作发生在隔离分支或隔离 worktree。

主控 agent 不应把模糊需求直接丢给 worker agent。

### 前端 Worker Agent

前端 worker agent 适合处理：

- React 组件拆分。
- CSS 和布局。
- 响应式修复。
- 图标、按钮、菜单、弹窗、工具栏等 UI 控件。
- Pencil / 截图 / HTML demo 到实际组件的还原。
- 视觉 polish。

前端 worker agent 不负责：

- Tauri command 设计。
- Rust 领域模型。
- 文件系统读写。
- 游戏目录识别。
- Mod 安装规则。
- 存档备份和恢复。
- GitHub Actions、hooks、policy、`.codex/skills/hmm*` 项目技能。

如果前端 worker 发现任务需要这些能力，必须停止并返回 `NEEDS_CONTEXT` 或 `BLOCKED`。

### 后端 Worker Agent

后端 worker agent 适合处理：

- Rust crate 内部实现。
- trait / interface 实现。
- use case 编排。
- 数据校验。
- 测试覆盖。

后端 worker 不能修改前端 UI 结构来规避接口问题，也不能把游戏 adapter 规则写进通用核心模块。

### 审查 Agent

审查 agent 必须只读审查，除非主控明确分配“修复审查问题”的任务。

审查分两类：

- **规格审查**：检查实现是否符合任务包和设计稿，是否多做或少做。
- **质量审查**：检查架构边界、文件职责、可维护性、测试、风险和安全性。

审查 agent 不应因为实现“看起来差不多”就通过。发现 Important 或 Critical 问题时，必须给出具体文件和行号。

## 分支与隔离

多 agent 协作必须使用隔离分支。

推荐命名：

```text
codex/<topic>
gemini/<topic>
agent/<topic>
```

规则：

- 不在 `main` 上直接修改。
- 不把无关任务混进同一分支。
- 如果 worker agent 需要独立空间，优先使用独立 worktree 或独立分支。
- 同一时间多个 worker 并行时，必须分配互不重叠的写入范围。
- 合并前由主控 agent 统一检查 `git status --short --branch`。

## 任务分发流程

标准流程：

```text
需求确认
  -> 主控 agent 拆任务
  -> 写任务包
  -> worker agent 实现
  -> worker 自测
  -> worker 提交或交付 patch
  -> 规格审查
  -> 质量审查
  -> 主控集成
  -> 本地验证
  -> PR / 人工视觉确认
```

任何步骤失败，都不能跳到后续步骤。

## 前端任务包模板

给前端 worker 的任务必须使用类似格式：

```markdown
# Frontend Agent Task Packet

## 任务目标
一句话说明要实现或修复的 UI。

## 视觉来源
- Pencil 设计稿：
- 截图路径：
- HTML demo：
- 关键视觉说明：

## 允许修改文件
- src/app/frame/Example.tsx
- src/app/frame/Example.css

## 禁止修改文件
- src-tauri/**
- src-tauri/crates/**
- .codex/**
- .github/**
- .githooks/**
- scripts/**
- policy/**
- package.json
- pnpm-lock.yaml
- eslint.config.js

## 必读文件
- AGENTS.md
- docs/ARCHITECTURE.md
- docs/TESTING.md
- docs/APPEARANCE_SYSTEM.md
- docs/APPEARANCE_EXTENSION_GUIDE.md
- 相关组件和样式文件

## 架构边界
本任务只处理展示层，不处理游戏目录、Mod 安装、存档、Tauri command 或 Rust 逻辑。

## UI 要求
- 布局：
- 状态：
- 交互：
- 响应式：
- 无障碍：
- 动效：

## 样式要求
- 使用 `src/shared/styles/tokens.css` 中的语义 token。
- 不为浅色/深色复制两套页面结构。
- 不在业务页面读取 shell variant。
- 局部组件样式使用组件命名空间。

## 验证命令
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build

## 完成汇报格式
- Status: DONE / DONE_WITH_CONCERNS / BLOCKED / NEEDS_CONTEXT
- 改动文件：
- 实现内容：
- 验证命令和结果：
- 未覆盖项：
- 风险或疑问：
```

任务包中的“允许修改文件”是硬边界。worker 不应因为方便而修改其他文件。

## 前端实现规则

前端 worker 必须遵守：

- 组件只处理展示、交互和局部 UI 状态。
- 业务规则来自 props、hook 或 API 返回，不在组件里推断游戏路径。
- 样式优先使用语义 token。
- 局部组件 CSS 使用清晰命名空间，例如 `.theme-menu`、`.sidebar-mode-control`。
- 图标优先使用项目已有图标库。
- 响应式布局使用 CSS grid、flex、minmax、容器约束等稳定方案。
- 文本不能溢出按钮、卡片或状态栏。
- UI 控件必须有清晰的 focus 状态。
- 菜单、弹窗、下拉框必须考虑关闭方式，例如外部点击和 `Escape`。
- 涉及视觉还原时必须提供截图或可复查的浏览器验证说明。

前端 worker 禁止：

- 在 Dashboard 或业务页面里按主题复制两套 DOM。
- 在业务页面里通过 `[data-sidebar-mode]` 分叉布局。
- 直接调用 Tauri 文件系统能力。
- 直接实现 Mod 安装、解压、备份、回滚规则。
- 为了视觉效果绕过 `tokens.css`，大面积硬编码颜色。
- 引入新的 UI 依赖而不说明原因。
- 修改 `package.json` 或 lockfile，除非任务明确允许。

## 后端与高风险任务规则

涉及以下区域时，不建议交给只擅长前端的 agent：

- 压缩包解压和路径校验。
- 游戏目录写入、覆盖、删除。
- 存档备份和恢复。
- 安装清单和回滚逻辑。
- Tauri command 暴露的文件操作。
- 并发任务、锁和取消。
- Steam library 扫描。
- 外部工具、loader、DLL 检测。

如果必须分配，应由主控 agent 提供更严格的任务包，并要求补充测试或说明无法测试的原因。

## 写入范围策略

任务包应尽量让写入范围互不重叠。

推荐拆分：

- `src/app/frame/*`：App frame、顶部栏、全局控件。
- `src/app/shell/*`：Shell、侧边栏、导航布局。
- `src/features/<feature>/*`：单个功能页面。
- `src/shared/components/*`：通用 UI 组件。
- `src/shared/styles/*`：全局 token 和 reset。

不推荐让一个 worker 同时修改：

- 前端 UI 和 Rust crate。
- 业务页面和治理脚本。
- 组件实现和发布文档。
- 测试配置和功能代码。

## 审查门禁

### 规格审查清单

规格审查必须核对：

- 是否只修改允许文件。
- 是否实现任务包列出的每项 UI / 行为。
- 是否遗漏响应式、无障碍或状态要求。
- 是否多做了未授权功能。
- 是否把设计稿中的临时内容当成正式功能。
- 是否恢复或保留了正确中文文案。
- 是否提交了截图、说明或可复查证据。

规格不通过时，worker 必须先修复，不能进入质量审查。

### 质量审查清单

质量审查必须核对：

- 文件是否职责单一。
- 新增文件是否过大。
- 组件是否只依赖公开接口。
- CSS 是否局部、可维护。
- 是否使用语义 token。
- 是否破坏前端边界检查。
- 是否引入硬编码路径、游戏名、资源编号或平台规则。
- 是否有明显可访问性问题。
- 是否有明显性能问题或不必要重渲染。
- 是否执行了要求的验证命令。

质量审查发现 Critical 或 Important 问题时，不能合并。

## 验证要求

最小验证以 `docs/TESTING.md` 为准。

前端任务通常至少执行：

```powershell
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

涉及 App Shell、Dashboard、侧边栏或外观系统时，还应执行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1
```

最终合并前优先执行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

如果某项验证不能执行，必须在汇报中写清楚原因。

## 失败处理

worker agent 返回状态必须使用：

- `DONE`：完成且自测通过。
- `DONE_WITH_CONCERNS`：完成但存在疑问或未覆盖风险。
- `NEEDS_CONTEXT`：缺少上下文，不能安全继续。
- `BLOCKED`：任务无法完成，需要主控拆分、改方案或人工决策。

主控 agent 处理规则：

- `DONE`：进入审查。
- `DONE_WITH_CONCERNS`：先阅读 concerns，再决定是否审查或补上下文。
- `NEEDS_CONTEXT`：补齐上下文后重新分配。
- `BLOCKED`：不要强行继续，先重新评估任务边界。

同一个问题连续失败三次，应停止尝试并重新讨论方案。

## PR 汇报模板

多 agent 协作产生的 PR 应说明：

```markdown
## 改动摘要

## 参与 Agent
- 主控：
- Worker：
- 审查：

## 任务包范围
- 允许修改文件：
- 实际修改文件：

## 验证
- [ ] cmd /c corepack pnpm run typecheck
- [ ] cmd /c corepack pnpm run lint
- [ ] cmd /c corepack pnpm run build
- [ ] powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1

## 视觉证据
- 截图：
- 浏览器 smoke test：

## 风险和未覆盖项
```

## 常见反模式

以下行为必须阻止：

- 让前端 agent 自己探索全仓库并自由修改。
- 把“看起来能用”的 UI 直接合并。
- 用一个 PR 同时改 UI、后端、脚本、文档和治理配置。
- 为了通过验证而降低 lint、测试或 policy 约束。
- 在没有截图或浏览器验证的情况下声称 UI 已还原。
- 把临时 demo、缓存、截图、真实路径提交到仓库。
- 让审查 agent 同时修改代码。
- 让 worker agent 在 review 未通过时继续做下一个任务。

## 推荐协作模式

推荐默认模式：

```text
Codex 主控拆任务
  -> Gemini 前端 worker 实现 UI
  -> Codex 或独立审查 agent 做规格审查
  -> Codex 或独立审查 agent 做质量审查
  -> 人工确认视觉效果
  -> verify 通过
  -> PR
```

Gemini 等前端强模型负责 UI 细节和视觉还原，Codex 负责架构边界、任务分解和验证门禁。CI、hooks 和脚本负责底线约束。三者缺一不可。
