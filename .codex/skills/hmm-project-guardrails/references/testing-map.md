# 验证命令速查

用本文件选择验证命令。最终依据仍是 `docs/TESTING.md`。

最终回复必须说明：

- 实际执行了哪些命令。
- 哪些命令未执行。
- 未执行原因。
- 高风险路径仍有哪些残余风险。

## 常用命令

统一验证：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

前端：

```powershell
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
cmd /c corepack pnpm run test
```

Rust / Tauri 桥接：

```powershell
cargo test --workspace
cargo check --workspace
```

Rust 核心逻辑：

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

前端 shell/layout 边界：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1
```

文档/治理：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-whitespace.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

## 按改动类型

- 仅文档：whitespace、doc links；涉及治理文件时跑治理相关检查或统一验证。
- 前端 UI：typecheck、lint、build；工作流改动补测试或 smoke。
- Tauri command/DTO：Rust tests/check；如果 API wrapper 变化，补前端 typecheck。
- Rust domain/app/infra/adapter：cargo test；共享行为变化时补 clippy。
- 安装/卸载/回滚：用临时目录覆盖新增文件、覆盖备份、失败回滚、manifest 卸载、冲突。
- 压缩包导入：人工最小包覆盖正常路径、路径穿越、绝对路径、大小写碰撞、伪装图片。
- 存档备份：只用临时存档目录；测试 manifest 和保留策略。
- 并发/任务系统：事件 task id、取消、写入串行、不留下半写 manifest。
- 日志/审计：路径、用户名、Steam ID、token、cookie 脱敏；高风险操作产生 audit event。

不要基于“应该能过”或旧运行结果声称通过。
