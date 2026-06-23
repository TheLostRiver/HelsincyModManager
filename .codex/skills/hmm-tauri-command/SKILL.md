---
name: hmm-tauri-command
description: Use when Helsincy Mod Manager work adds or changes Tauri commands, Rust DTOs, AppState wiring, task events, custom protocols, frontend typed API wrappers, command errors, or frontend/backend contract documentation.
---

# HMM Tauri Command

## 概览

保持 Tauri 边界薄。Commands 只校验输入形状、转换 DTO、调用 `AppState` services、返回稳定错误并发出受控事件；领域规则和文件系统安全留在 app/ports/infra 后面。

HMM 专属 skills 属于本仓库 `.codex/skills/`，不属于全局 skill 目录。

## 必读上下文

编辑前，读或扫描项目基础文档：

- `AGENTS.md`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/ROADMAP.md`
- `CONTRIBUTING.md`
- `docs/TESTING.md`
- `docs/GOVERNANCE.md`
- `SECURITY.md`

然后读取 Tauri 边界上下文：

- `docs/FRONTEND_BACKEND_CONTRACT.md`
- `.codex/skills/hmm-project-guardrails/references/frontend-backend-boundary.md`

如果 command 触及真实文件操作、import、install、backup、rollback、preview-image cache、logs、diagnostics、audit events、redaction 或 save data，还要读 `docs/LOGGING.md` 并使用 `hmm-install-safety`。

## 边界路由

- 如果改动新增或更新 feature-local typed API wrappers、React task listeners、frontend state、thumbnail display 或浏览器可见 workflow，同时使用 `hmm-frontend-workflow`。
- 如果改动 AppState services、Rust crate placement、app/ports/infra dependency direction、repositories、game adapters 或 DTO/domain mapping，同时使用 `hmm-rust-crate-boundary`。
- 如果改动 task identity、task events、long-running work、cancellation、progress phases、result refs、locks、queues 或 database/write serialization，同时使用 `hmm-task-and-concurrency`。
- 如果改动真实文件操作、import、install、backup、rollback、logs、diagnostics、audit events、redaction、save data、preview-image cache containment 或任何 data-safety flow，同时使用 `hmm-install-safety`。

## Command 形态

1. Command 用 `snake_case`，按用例命名，不按原始文件系统操作命名。
2. 跨边界 Rust DTO 放在 `src-tauri/src/`，struct 使用 `#[serde(rename_all = "camelCase")]`。
3. 前端分支判断使用稳定的 `snake_case` enum/error 值。
4. Command body 保持小：解析/校验输入、从 `State<'_, AppState>` 调 app service、映射 output/error DTO。
5. 新增或更新 feature-local frontend typed API wrappers；`src/shared/api/tauri.ts` 只做通用 helper/re-export，不做 feature dumping ground。
6. 长任务返回受控 task identity，并发出 `hmm://task-progress` events，包含 `taskId`、稳定 `status`、已注册 `phase` codes，以及 `resultRef` 或最终结果查询 command；不要把巨大结果或原始路径塞进 progress events。
7. Command names、DTO shapes、error codes、phase codes 或 typed API contracts 变化时，更新 `docs/FRONTEND_BACKEND_CONTRACT.md`。
8. 对 thumbnails 等 custom protocols，只暴露 opaque refs 支撑的受控 URL；handler 必须留在受控 app cache/storage roots 内，拒绝 traversal/absolute/symlink access，设置 content type，并避免在 DTO、logs 和 frontend code 中泄漏真实磁盘路径。

## 禁止形态

- `copy_file`、`delete_path`、`read_any_file`、`write_any_file` 或其他宽泛 filesystem commands。
- 前端提交最终 install paths、retarget 后的 `nativePC` paths、cache paths、thumbnail disk paths 或 game adapter internals。
- Command 直接实现 install、backup、rollback、game directory probing 或 MHW parsing，而不是调用 app services。
- 用户可见 error message 包含完整本地路径、用户名、Steam IDs、tokens、cookies、真实 save content 或第三方 Mod content。
- 基于“只有一个任务活跃”来匹配 event；task identity 必须显式。
- 在未先明确更新 `docs/FRONTEND_BACKEND_CONTRACT.md` 的情况下，把 `convertFileSrc`、asset protocol、base64 data URLs 或 raw cache paths 作为正式 thumbnail/resource contract。

## 验证

Review 时使用 `references/tauri-command-checklist.md`。最小检查取决于范围：

| 改动 | 最小检查 |
| --- | --- |
| 仅 command parser/DTO | `cargo test --workspace` 和 `cargo check --workspace`；可行时补 Rust unit tests 覆盖 validation 和 DTO serialization shape。 |
| Frontend typed API | Frontend typecheck，加聚焦测试确认 wrapper 调用预期 command 且避开禁止的 path APIs。 |
| Event/long task | `cargo test --workspace` 和 `cargo check --workspace`；Rust tests 覆盖 `taskId`、kind/status/phase mapping；触及时覆盖 frontend listener behavior。 |
| File/safety command | `hmm-install-safety` 检查加 bridge tests；不使用真实 game/save directories。 |
| Custom protocol | Handler tests 覆盖 opaque refs、content type、cache-root containment、traversal/absolute/symlink rejection；DTO/logs/frontend 不出现真实 cache paths。 |
| Contract change | 更新文档并运行项目验证。 |

最终交付前优先完整运行 `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1`，尤其是 command 跨 frontend 和 Rust 时。任何最小检查未运行，都要在 final handoff 里明确原因。

## 常见错误

- 让 command 代码变成 application service。
- 因为 UI 需要展示某些内容，就返回方便的 raw paths。
- 把新的 feature call 塞进 shared API barrel，而不是 feature-local wrapper。
- 用 `message` 文本做前端逻辑，而不是稳定 `code` 值。
- 忘记在 contract doc 注册 task phase codes。
- 通过临时 file URLs 服务 thumbnails，而不是使用文档化的 custom protocol contract。
