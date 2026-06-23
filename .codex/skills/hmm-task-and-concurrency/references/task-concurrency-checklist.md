# Task and Concurrency Checklist

用于 HMM task manager、event、cancellation、lock、queue 和 concurrent workflow changes。

## Task Identity

- Start command 返回 task id 和稳定 kind/status。
- 每个 event 携带 `taskId`。
- Event name 和 payload 匹配 `docs/FRONTEND_BACKEND_CONTRACT.md`。
- Phase code 稳定、已文档化，且不从用户可见文本推断。
- Large results 使用引用，不嵌入 progress events。

## 边界路由

- Rust crate placement、dependency direction、AppState services、repositories 或 DTO/domain mapping changes 也使用 `hmm-rust-crate-boundary`。
- Command/event DTO、custom protocol 或 frontend/backend contract changes 也使用 `hmm-tauri-command`。
- React task UI、frontend listeners、typed API wrappers、task state 或 browser-visible workflow changes 也使用 `hmm-frontend-workflow`。
- File write/delete/backup/restore/install/uninstall/rollback changes 也使用 `hmm-install-safety`。

## Cancellation

- Queued/running/completed/failed/cancelled states 明确。
- Cancellation 有 safe points 和 deterministic results。
- Unknown task 和 non-cancellable states 返回稳定 errors。
- Frontend listener 按 `taskId` reconcile events。

## Locks 和 Queues

- 同 game instance writes 串行。
- 同 profile enable/disable/install/uninstall operations 串行。
- Prepare work 在 game write locks 外运行。
- Database write transactions 保持短。
- Error/cancel paths 会释放 locks。

## Logging 和 Audit

- Task logs 和 progress 共享同一 task id。
- 用户可见 messages 不包含 raw paths 或 sensitive content。
- Write/overwrite/delete/backup/restore/manifest/rollback operations 在对应路径存在时发出 Audit Log entries。
- `RollbackFailed` 和 `DataSafetyRisk` 类失败可审计。

## 验证

- Rust tests 覆盖 task id propagation 和 phase/status mapping。
- Rust task/concurrency changes 运行 `cargo clippy --workspace --all-targets -- -D warnings`，除非有文档化原因阻止。
- Concurrency tests 使用 fake services 或 temp fixtures。
- 自动测试不要求真实 game directories、真实 saves 或第三方 Mod packages。
- 如果 UI task behavior 改变，bridge/frontend tests 覆盖 listener matching 和 `hmm-frontend-workflow` 检查。
