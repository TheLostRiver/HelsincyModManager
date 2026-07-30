# 风险分级验证

用本文件选择验证命令。最终依据仍是 `docs/TESTING.md`。

最终回复必须说明：

- 实际执行了哪些命令。
- 哪些命令未执行。
- 未执行原因。
- 高风险路径仍有哪些残余风险。

## 常用命令

以下是按场景选择的命令，不是每次改动都要顺序执行的固定套餐。

中高风险 PR candidate 的统一验证：

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

Rust / Tauri 全 workspace 候选验证：

```powershell
cargo test --workspace
cargo check --workspace
```

Rust 全 workspace 候选验证：

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

前端 shell/layout 边界：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-frontend-boundaries.ps1
```

文档/治理聚焦检查：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-whitespace.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-doc-links.ps1
```

## 按改动类型

- 仅文档：whitespace、doc links；涉及治理文件时跑 policy/secret 等聚焦检查，并在首次 PR ready 前按
  风险决定是否运行统一验证。
- 前端 UI：选择 typecheck、lint、相关 test 或 smoke；bundling/asset/build config 变化时补 build。
- Tauri command/DTO：运行 touched command/DTO 的 Rust contract tests；API wrapper 变化时补前端检查。
- Rust domain/app/infra/adapter：运行 touched crate/module tests；共享行为或依赖边界变化时在候选阶段
  由完整 verify 覆盖 workspace clippy。
- 安装/卸载/回滚：用临时目录覆盖新增文件、覆盖备份、失败回滚、manifest 卸载、冲突。
- 压缩包导入：人工最小包覆盖正常路径、路径穿越、绝对路径、大小写碰撞、伪装图片。
- 存档备份：只用临时存档目录；测试 manifest 和保留策略。
- 并发/任务系统：事件 task id、取消、写入串行、不留下半写 manifest。
- 日志/审计：路径、用户名、Steam ID、token、cookie 脱敏；高风险操作产生 audit event。

不要基于“应该能过”或旧运行结果声称通过。
