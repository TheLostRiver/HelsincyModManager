# Tauri Command Checklist

新增或修改 HMM Tauri commands、DTOs、task events、custom protocols 或 frontend typed APIs 时使用此 checklist。

## 边界

- Command name 描述用例，不描述原始 filesystem primitive。
- Command body 校验输入、映射 DTO、调用 `AppState`，并返回 DTO/errors。
- Domain decisions 放在 `hmm-core`/`hmm-app`；真实 I/O 放在 `hmm-ports` 和 `hmm-infra` 后面。
- MHW-specific parsing 放在 `hmm-games-mhw`，不放在 command code 或 generic frontend code。

## DTO

- 跨 Tauri 的 Rust structs 使用 `#[serde(rename_all = "camelCase")]`。
- 跨 Tauri 的 Rust enum values 使用稳定字符串，通常为 `snake_case`。
- TypeScript DTO types 匹配实际 JSON shape。
- DTOs 暴露 display labels、ids、summaries 或 controlled URLs，不暴露 raw sensitive paths。
- `metadata` 是 display/context data；前端不得从中推导 install paths。

## 错误

- Error `code` 稳定，适合前端测试。
- 用户可见 `message` 不用于逻辑。
- Errors 避免 full local paths 和 sensitive data。
- Contract 有对应字段时，高风险 commands 包含足够 category/recoverability detail。

## Events 和 Long Tasks

- 启动 command 返回 `TaskStartedDto` 或等价 controlled identity。
- Progress events 使用文档化的 `hmm://task-progress` contract。
- 每个 progress event 携带 `taskId`、稳定 kind/status values 和已注册 phase。
- Phase codes 使用文档化的 `<task_kind>.<stage>.<sub>` 风格，并注册在 `docs/FRONTEND_BACKEND_CONTRACT.md`。
- 大型最终结果通过 result reference 或 query command 获取，不塞进 progress events。
- Cancellation 和 result lookup 遵循文档化 task contract，而不是 page-local assumptions。

## Custom Protocols

- Thumbnail/resource URLs 是 opaque refs 支撑的 controlled protocol URLs，绝不是 raw disk paths。
- Handlers 校验 cache/storage root containment，并拒绝 traversal、absolute paths、symlinks 和 unregistered refs。
- Handlers 设置准确的 `Content-Type` 和 cache behavior。
- DTOs、logs 和 frontend code 不暴露真实 cache paths 或 thumbnail file extensions。

## Frontend

- 新增或更新 feature-local API wrapper。
- Shared invoke helper 只用于通用机制。
- 除非 contract 明确允许，不使用 `convertFileSrc`、asset protocol、raw cache paths 或 arbitrary local path reads。
- View models 将 DTOs 映射为 UI state，不重建 backend rules。

## 验证

- 接受 paths 时，parser 拒绝 empty/relative/invalid inputs。
- DTO serialization 或 source tests 覆盖 shape 和 command names。
- Frontend tests 覆盖敏感流程的 wrapper command names 和 forbidden APIs。
- Tauri/Rust bridge changes 至少运行 `cargo test --workspace` 和 `cargo check --workspace`；否则 final handoff 说明无法运行原因。
- Contract、governance 或 `.codex/` changes 可行时运行项目 verification script，并指出预期需要 human review。
