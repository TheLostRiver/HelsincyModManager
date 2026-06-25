# InstallPlan MVP 下一切片待办

> 本文是 `docs/superpowers/plans/2026-06-19-mod-installation-mvp-implementation.md` 的短切片待办，用来约束下一条 InstallPlan PR 的范围。完整安装 MVP 蓝图仍以后者为准。

## 当前基线

- Mod 导入分析、预览图处理、导入结果持久化和 Mod 库查询已经落地。
- 前端 Mod 库已经能消费后端 `get_mod_library()`，并且后端成功返回空数组时不再显示 mock 数据。
- InstallPlan 领域模型、只读 app 预览服务和 `preview_install_plan` Tauri DTO/command 已经落地。
- 安装链路尚未落地；任何真实游戏目录写入仍必须等待 `InstallPlan -> backup -> commit -> manifest -> rollback/recover` 链路补齐。

## 当前切片 TODO

- [x] 在 `hmm-core` 定义最小 `InstallPlan`、目标路径校验和冲突模型。
- [x] 在 `hmm-app` 增加只读安装计划预览服务。
- [x] 增加 `preview_install_plan` Tauri DTO/command，并更新前后端契约。
- [x] 让后端从已导入 Mod 的受控 sandbox 和游戏 adapter 生成安装计划输入，减少正式前端直接传 `targetPath` 的需要。
- [x] 接入最小前端 typed API / 预览 UI。
- [ ] 在 backup / manifest / rollback 链路补齐后，再进入真实安装提交。

## 当前 PR 目标

实现后端驱动的只读 InstallPlan 预览输入切片：

- 通过已持久化导入结果按 `modId` 找到 `packageId`。
- 通过受控 sandbox locator 定位已导入包的 sandbox root。
- 只读枚举 sandbox 内普通文件，生成 package file candidate。
- 由 game adapter 提供允许安装根，app service 组装 `InstallPlan` 输入。
- 增加 `preview_imported_mod_install_plan` Tauri 入口，让正式前端不再直接提交 `targetPath`。

## 明确不做

- 不写入真实游戏目录。
- 不实现安装提交、备份、manifest 写入、卸载或回滚。
- 不做重定向 staging。
- 不新增完整安装 UI。
- 不让前端拼接安装路径、推断 MHW 路径规则或承担文件系统安全规则。
- 不使用真实第三方 Mod 包、真实 MHW 安装目录或真实玩家存档做测试。

## 建议文件边界

- `src-tauri/crates/hmm-core/src/install.rs`
  - `ModId`
  - `ProfileId`
  - `PackageFileId`
  - `InstallTargetPath`
  - `InstallPlan`
  - `InstallAction`
  - `InstallConflict`
  - 纯领域校验和冲突规则
- `src-tauri/crates/hmm-core/src/lib.rs`
  - 导出安装领域模型
- `src-tauri/crates/hmm-app/src/install.rs`
  - `BuildInstallPlanRequest`
  - `InstallPlanningService`
  - 编排适配器允许根与文件提供者，不依赖具体基础设施实现
- `src-tauri/crates/hmm-app/src/lib.rs`
  - 导出安装计划服务

第一条 PR 可以先不新增 `hmm-ports`。如果实现时需要把文件提供者或适配器能力抽成 trait，再单独引入最小 trait，并保持 `hmm-app` 只依赖 trait。

## 安全约束

- `hmm-core` 不能感知 `nativePC`、`plNNN_VVVV`、`f_equip`、`m_equip` 或任何 MHW 专属路径语义。
- MHW 允许的安装目标根应由适配器层提供；核心层只消费字符串形式的允许根，不对具体值做分支。
- 计划预览阶段必须只读，不创建目录、不复制文件、不删除文件、不写 manifest。
- 后续真实写入必须从本切片产出的 `InstallPlan` 消费，不能另开直接复制的快捷路径。

## 最小验收测试

- `InstallTargetPath` 接受普通相对路径。
- `InstallTargetPath` 拒绝：
  - 空路径
  - 绝对路径
  - `..` 父级穿越
  - Windows 盘符前缀
  - 适配器未允许的目标根
- `InstallPlan` 对同一目标路径的多来源文件报告阻断冲突。
- 带显式优先级的多来源文件可以生成有序动作，且仍保留可解释的文件层信息。
- 计划生成不访问真实文件系统。

## 验证命令

下一条 PR 至少执行：

```powershell
cargo test --workspace
cargo check --workspace
```

如果新增 Tauri command 或前端类型化 API，再补充：

```powershell
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

最终交付前优先执行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

## 后续切片顺序

1. InstallPlan 领域模型与只读计划预览。
2. 安装提交服务、JSON manifest 仓储、备份和失败回滚，测试只使用临时游戏目录。
3. Tauri `preview_install_plan` / 后续 `start_install_task` 命令和 DTO。
4. 前端类型化 API 与最小计划预览 UI。
5. ARMOR_RETARGET staging 接入 InstallPlan 输入。
