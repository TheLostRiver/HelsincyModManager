# Rust Crate Boundary Checklist

用于 HMM Rust crate placement、dependency direction 和 workspace 工作。

## 放置

- 新 type/function 放在最窄且正确的 crate。
- Domain concepts 保持纯净，不依赖 Tauri、FS、DB、Steam 或 platform APIs 也可测试。
- Ports 是 traits/interfaces，不泄漏 concrete infra。
- App services 通过 ports 编排 use cases。
- Infra 包含 I/O details，但不包含跨域 domain policy。
- Game-specific rules 放在 `hmm-games-*`。

## 依赖方向

- `hmm-core` 不依赖 app/ports/infra/games/Tauri。
- `hmm-app` 依赖 ports 和 domain，不依赖 infra concretes。
- Tauri shell 向外依赖 app/state wiring，并显式映射 DTOs。
- Tauri/DTO bridge changes 已对照 `docs/FRONTEND_BACKEND_CONTRACT.md` 检查。
- 新 shared helpers 不为了方便而反转依赖。

## Game Adapter

- MHW rules 留在 `hmm-games-mhw`。
- 未来游戏逻辑不在 MHW adapter 内部分支。
- 自动测试不要求真实 MHW install。
- Catalog/rule additions 在可行时 data-driven。

## 安全和错误

- File writes 仍通过 plan/manifest/backup/rollback 设计。
- Errors 为 command mapping 保留稳定 codes/categories。
- Sensitive raw paths 或 player data 不进入 logs 或 UI DTOs。

## Tasks 和 Concurrency

- TaskManager、task events、cancellation、progress phases、locks、queues 和 database/write serialization
  改动同时读取 `task-concurrency-checklist.md`。
- 长时间 scan/hash/extract/analyze work 留在 game write locks 外。
- 同 game instance writes 和同 profile enable/disable/install/uninstall paths 串行。
- Progress 和 task logs 携带显式 task identity。

## 验证

- 开发期间优先运行 touched crate/module 的聚焦 tests、check 或 clippy。
- 跨 crate/public contract 变化在首次 PR ready 前按 router 风险分级运行完整 `verify.ps1`，由统一入口
  覆盖 workspace test/check/clippy；不要在每个 commit 后手工重复同一全量命令。
- 即使改动只在单个 crate 内，只要触及安装/存档写入、回滚、并发、安全或其他高风险边界，也必须在
  首次 PR ready 前运行完整 `verify.ps1`。
- 低风险 crate-local 改动若不运行完整入口，记录实际聚焦命令和省略原因；required CI 仍必须成功。
- Task/concurrency changes 已按触及范围运行 task identity、phase codes、cancellation state、lock/queue ordering 或 database write serialization 聚焦检查。
- 边界变化时已检查 architecture docs。
