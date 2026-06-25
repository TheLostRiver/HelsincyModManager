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
- [x] 增加安装提交服务、JSON manifest 仓储、备份和失败回滚的后端骨架，测试只使用临时目录。
- [ ] 接入任务、写锁、审计日志和 Tauri `start_install_task` 后，再开放真实安装提交入口。

## 当前 PR 目标

实现后端安装提交安全骨架：

- 在 `hmm-core` 定义最小 `InstallManifest` / manifest entry。
- 在 `hmm-ports` 增加安装提交所需的窄接口：源文件读取、游戏文件写入、备份存储和 manifest 仓储。
- 在 `hmm-app` 增加 `InstallCommitService`，按 `InstallPlan -> backup -> commit -> manifest` 顺序编排。
- manifest 写入失败时，按已应用动作反向 rollback：新文件删除、覆盖文件恢复旧内容。
- 在 `hmm-infra` 增加受 root containment 约束的文件系统适配器和 JSON manifest 仓储。

## 明确不做

- 不写入真实游戏目录。
- 不接 Tauri `start_install_task`，不从前端触发安装提交。
- 不实现卸载、崩溃恢复扫描或跨进程恢复。
- 不实现 game/profile 写锁和任务进度事件。
- 不实现安装审计日志；真实入口接入前必须补上 Audit Log。
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
  - `InstallCommitService`
  - `CommitInstallPlanRequest`
  - 提交阶段只依赖 ports，不依赖具体文件系统
- `src-tauri/crates/hmm-ports/src/install.rs`
  - `InstallSourceFileReader`
  - `InstallGameFileSystem`
  - `InstallBackupStore`
  - `InstallManifestRepository`
- `src-tauri/crates/hmm-infra/src/install_commit.rs`
  - 文件系统源文件读取器
  - 文件系统游戏目录写入适配器
  - 文件系统备份存储
  - JSON manifest 仓储
- `src-tauri/crates/hmm-app/src/lib.rs`
  - 导出安装计划服务

## 安全约束

- `hmm-core` 不能感知 `nativePC`、`plNNN_VVVV`、`f_equip`、`m_equip` 或任何 MHW 专属路径语义。
- MHW 允许的安装目标根应由适配器层提供；核心层只消费字符串形式的允许根，不对具体值做分支。
- 计划预览阶段必须只读，不创建目录、不复制文件、不删除文件、不写 manifest。
- 后续真实写入必须从本切片产出的 `InstallPlan` 消费，不能另开直接复制的快捷路径。
- 文件系统适配器必须把 package file id 和 target path 解析为受控 root 下的相对路径，拒绝父级穿越、绝对路径和 Windows 盘符前缀。
- 在 Tauri 真实入口接入前，必须补上 game/profile 写锁、任务事件和 Audit Log。

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
- 提交计划写入新文件后保存 manifest。
- 覆盖已有文件前先写备份，并在 manifest entry 中记录 backup ref。
- manifest 保存失败时回滚已写文件：新文件删除，旧文件恢复。
- 文件系统 source reader 拒绝 package file id 父级穿越，错误消息不泄露临时根路径。
- JSON manifest 内容不包含临时测试根路径。

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
2. 后端驱动的导入 Mod 安装计划输入。
3. 前端类型化 API 与最小计划预览 UI。
4. 安装提交服务、JSON manifest 仓储、备份和失败回滚，测试只使用临时目录。
5. Tauri `start_install_task` 命令、任务事件、写锁和 Audit Log。
6. ARMOR_RETARGET staging 接入 InstallPlan 输入。
