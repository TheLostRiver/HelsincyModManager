# InstallPlan MVP 待办

本文档维护 `InstallPlan` / Mod 安装 MVP 的后续切片、验收标准和安全门禁。它不是一次性 PR 计划，而是安装能力继续推进时的任务入口。

当前实现事实以 [InstallPlan 模块现状](INSTALL_PLAN_STATUS.md) 为准；长期方案和可选后端设计参考 [Mod 安装方案规划](mod_installation_strategy.md)；前后端通信形状参考 [前后端通信契约](FRONTEND_BACKEND_CONTRACT.md)。

## 目标

MVP 的目标不是一次性完成所有安装管理能力，而是先形成一条可测试、可审计、可回滚的最小安全链路：

```text
已导入 Mod
  -> 受控 sandbox
  -> 后端重建 InstallPlan
  -> 冲突和前置条件检查
  -> 用户确认
  -> 安装任务
  -> backup / commit / manifest
  -> 失败回滚或恢复提示
```

所有后续切片都必须保持这个边界：

- 前端只展示后端返回的状态和摘要，不拼接安装路径。
- Tauri command 只接收内部 id、用户选择和受控参数，不接收真实目录或最终目标路径。
- `hmm-core` 不感知 `nativePC`、MHW slot、retarget catalog 或真实文件系统。
- 真实游戏目录写入只能发生在提交服务或其后续受控执行器内。
- 卸载、修复和恢复只能基于 manifest、备份记录和受控审计信息，不根据当前 Mod 包重新猜测。

## 当前基线

已经落地：

- Mod 导入分析、预览图处理、导入结果持久化和 Mod 库查询。
- 前端 Mod 库消费 `get_mod_library()`；后端返回空数组时不再显示 mock 数据。
- `InstallPlan` 领域模型、目标路径校验、冲突模型和只读计划预览。
- 后端驱动的 `preview_imported_mod_install_plan`，从已导入 Mod 的受控 sandbox 和 game adapter 生成计划输入。
- 最小前端 typed API 和计划预览 UI。
- 安装提交服务、JSON manifest 仓储、备份和失败回滚骨架。
- Tauri `start_install_task`、`TaskKind::Install`、安装任务事件、game/profile 写锁和最小 Audit Log。

仍未完成：

- 完整安装 UI 工作流。
- manifest 查询和“已安装状态”展示。
- 基于 manifest 的卸载。
- 跨进程崩溃恢复扫描。
- ARMOR_RETARGET staging 接入 InstallPlan。
- rich manifest 字段和状态机。
- dependency/preflight 阻断。

## 已完成切片记录

以下切片已经完成，后续工作不应重复开同类 PR，除非是在修 bug 或补边界：

- [x] `hmm-core` 最小 `InstallPlan`、目标路径校验和冲突模型。
- [x] `hmm-app` 只读安装计划预览服务。
- [x] `preview_install_plan` Tauri DTO/command 与契约文档。
- [x] 后端从已导入 Mod 的受控 sandbox 和 game adapter 生成安装计划输入。
- [x] 前端 feature-local typed API 与最小计划预览 UI。
- [x] 安装提交服务、JSON manifest 仓储、备份和失败回滚骨架。
- [x] 安装任务入口、写锁、审计日志和 `start_install_task`。

## 后续切片优先级

### P0：最小安装 UI

目标：让用户能从 Mod 库触发安装任务，并看到任务进行中、成功、失败和取消状态。

范围：

- 在 Mod 库或详情页提供受控安装入口。
- 调用 `start_install_task`，只提交 `gameId`、`modId`、`profileId` 和 layer 摘要。
- 按 `taskId` 订阅任务事件，不用“当前页面只有一个任务”推断任务归属。
- 展示 `install.queued`、`install.plan.building`、`install.commit.processing`、`install.completed`、`install.failed`、`install.cancelled`。
- 安装失败时展示可读错误状态，不展示真实路径、sandbox 路径、manifest 正文或第三方 Mod 内容。

明确不做：

- 不新增卸载。
- 不实现 retarget。
- 不让前端构造 `targetPath`。
- 不在前端根据 MHW 规则判断文件是否可安装。

验收标准：

- 前端 typed API 不包含路径字段。
- 任务状态严格按 `taskId` 匹配。
- UI 能区分 failed 和 cancelled。
- 前端 typecheck、lint、build 通过。
- Rust command/DTO 测试仍通过。

### P0：Manifest 查询与安装状态摘要

目标：让前端能展示某个 profile / mod 的安装状态，但不暴露 manifest 文件路径或原始 manifest 正文。

范围：

- 增加后端查询服务，读取受控 manifest 仓储。
- 提供窄 Tauri command，例如按 `profileId` / `modId` 查询安装摘要。
- DTO 只返回状态、动作数量、冲突摘要、可恢复状态和必要的短 id。
- 前端展示“未安装 / 已安装 / 需要修复 / 状态未知”等摘要。

明确不做：

- 不把 manifest 文件路径返回给前端。
- 不返回完整本地路径。
- 不把 manifest 当作日志替代品。

验收标准：

- command 不接受路径参数。
- 查询失败使用稳定错误码。
- DTO 不含备份路径、manifest 路径或 sandbox/cache 路径。
- 文档同步更新 `FRONTEND_BACKEND_CONTRACT.md`。

### P1：基于 manifest 的卸载

目标：提供第一版安全卸载能力，删除或恢复本工具安装过的文件。

范围：

- 根据 manifest entries 计算卸载计划。
- 对本工具新增的文件执行删除。
- 对覆盖过的文件使用 backup ref 恢复。
- 对未知或不一致状态给出阻断或修复提示。
- 写入 Audit Log。

明确不做：

- 不根据当前 Mod 包重新猜测安装过什么。
- 不删除 manifest 未记录的文件。
- 不做批量 profile 切换。

验收标准：

- 覆盖“新增文件卸载”“覆盖文件恢复”“backup 缺失阻断”“manifest 不一致阻断”。
- 只使用临时目录或 fake file system 测试。
- 卸载失败不会留下误导性的 completed 状态。

### P1：崩溃恢复扫描

目标：启动或进入安装页时发现半完成安装，并给出可恢复、可重试或人工处理的明确状态。

范围：

- 扫描 manifest、备份记录和任务状态摘要。
- 识别已完成、需要 rollback、需要 repair、无法判断等状态。
- 提供后端 command 返回恢复摘要。
- 前端展示恢复入口或人工处理提示。

明确不做：

- 不自动删除未知文件。
- 不依赖当前 Mod 包内容猜测恢复动作。
- 不把 Task Log 当作唯一事实来源。

验收标准：

- 恢复判断来源清晰：manifest、backup、task state、审计摘要。
- 无法安全恢复时阻断并提示人工处理。
- 不输出本地真实路径。

### P1：ARMOR_RETARGET staging 接入

目标：让 retarget materialize 产物作为受控 provider 输入 `InstallPlan`，而不是绕过安装链路。

范围：

- retarget 只写 staging，不写游戏目录。
- `InstallPlan` 看到的是 retarget 后最终目标相对路径。
- 冲突检测基于最终目标路径。
- manifest 记录必要的 replacement binding snapshot。

明确不做：

- 不把 MHW slot parsing 放进通用 core。
- 不把 retarget 产物当成事实来源。
- 不让前端拼接 `nativePC` 或 `plNNN_VVVV` 路径。

验收标准：

- 原始导入包保持只读。
- staging 可丢弃、可重建。
- 事实来源仍是原始包 metadata、ReplacementBinding 和 InstallManifest。

### P2：Rich manifest 与状态机

目标：把当前 MVP manifest 扩展为可支撑卸载、恢复、修复、retarget 和后续虚拟映射的事实记录。

候选字段：

- `manifest_id`
- `game_id`
- `game_instance_id`
- `profile_id`
- `mod_id`
- `backend`
- `status`
- `created_at`
- `completed_at`
- `files`
- `backups`
- `replacement_bindings`
- `plan_hash`

候选状态：

- `planned`
- `committing`
- `completed`
- `rollback_required`
- `rolled_back`
- `repair_required`

验收标准：

- 旧 manifest 能被迁移或兼容读取。
- 状态变更有测试覆盖。
- 失败状态不会被误报为已完成。

### P2：Dependency / preflight

目标：安装提交前检查必需前置、风险文件、loader 要求和 profile 冲突。

范围：

- 后端基于已导入 metadata、adapter 规则和 manifest 摘要判断。
- 缺失必需前置时阻断安装。
- 可选前置或弱风险给出警告。
- 前端只展示后端给出的结构化结果。

明确不做：

- 不让前端自行匹配依赖。
- 不根据展示名直接判定已安装。
- 不把 dependency graph 查询结果升级成安装事实。

验收标准：

- 必需前置缺失可阻断。
- warning 和 blocking conflict 明确区分。
- 文档同步说明错误码和 UI 行为。

## 文件边界

后续切片应优先保持以下边界：

- `src-tauri/crates/hmm-core/src/install.rs`：领域模型、目标路径校验、冲突规则。
- `src-tauri/crates/hmm-app/src/install.rs`：计划生成、提交编排、manifest/backup/rollback 用例。
- `src-tauri/crates/hmm-app/src/install_task.rs`：安装任务、阶段事件、写锁、审计编排。
- `src-tauri/crates/hmm-ports/src/install.rs`：安装 source reader、game filesystem、backup store、manifest repository trait。
- `src-tauri/crates/hmm-infra/src/install_commit.rs`：文件系统实现和 root containment。
- `src-tauri/src/install_commands.rs`：窄 Tauri command 和 DTO 映射。
- `src/features/mods/`：Mod 管理 feature-local typed API 和 UI 状态展示。

不应新增的捷径：

- `copy_file` / `delete_path` / `write_any_file` 这类宽泛 Tauri command。
- 前端传入 `targetPath`、game root、backup root、manifest root 或 sandbox/cache 路径。
- 通用 core 识别 MHW 专属路径语义。
- 在 install executor 之外写入、覆盖或删除游戏目录文件。

## 安全门禁

每个切片都必须确认：

- 是否可能触碰真实游戏目录写入。
- 是否可能覆盖、删除或恢复文件。
- 是否影响 manifest、backup 或 rollback。
- 是否影响 task event、Audit Log 或错误码。
- 是否影响 frontend/backend contract。
- 是否涉及 retarget、staging 或 MHW adapter 规则。

只要涉及真实写入、卸载、恢复、retarget staging 或 manifest 状态机，就必须补充聚焦测试；无法测试时必须说明原因和风险边界。

## 验证要求

文档改动最小验证：

```powershell
git diff --check
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-whitespace.ps1
```

涉及 Rust 安装链路：

```powershell
cargo test --workspace
cargo check --workspace
```

涉及前端 API 或 UI：

```powershell
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

最终交付前优先执行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

## PR 描述建议

涉及 InstallPlan 的 PR 至少说明：

- 本 PR 对应本文档哪个切片。
- 是否触碰真实游戏目录写入。
- 是否改变 command / DTO / task phase / error code。
- 是否改变 manifest、backup、rollback 或 Audit Log。
- 已执行哪些验证。
- 哪些能力仍明确不做。
