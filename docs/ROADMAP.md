# 路线图

## Phase 0：架构基线

- 初始化仓库。
- 沉淀架构、模块边界和 MVP 范围。
- 沉淀协作、安全、测试、发布相关文档。
- 确认第一版技术栈。
- 确认首个游戏适配目标：《怪物猎人：世界 冰原》。

## Phase 1：项目脚手架

- 创建 Tauri 2 应用脚手架。
- 接入 React + TypeScript 前端。
- 创建 Rust workspace crates。
- 加入格式化、lint 和基础 CI。
- 添加 SQLite migration 基础结构。
- 落地统一验证脚本和发布脚本骨架。

## Phase 2：MHW:I MVP Core

- 实现游戏目录检测。
- 实现手动选择游戏目录。
- 实现压缩包检查和沙盒解压。
- 实现 `nativePC`、DLL、图片、readme 检测。
- 实现安全的预览图提取。
- 实现分类和标签存储。
- 实现安装计划生成。
- 实现带安装清单和基础回滚能力的安装执行器。
- 实现基础冲突检测。
- 实现手动存档备份。
- 实现一键启动游戏。

InstallPlan 当前落地状态见 [InstallPlan 模块现状](INSTALL_PLAN_STATUS.md)，后续切片见 [InstallPlan MVP 待办](INSTALL_PLAN_MVP_TODO.md)。

## Phase 3：玩家工作流

- 添加 Profile 支持。
- 添加前置依赖规则 catalog。
- 添加缺失前置警告。
- 添加自动存档备份调度。
- 添加 Mod 批量启用 / 禁用。
- 添加任务进度和取消 UI。

## Phase 4：替换目标映射

- 添加 MHW:I 官方目标 catalog。
- 添加外观替换映射。
- 添加武器替换映射。
- 添加语音替换映射。
- 添加感知绑定关系的冲突检测。
- 添加 retarget staging 工作流。

## Phase 5：跨平台准备

- 添加 Linux 路径抽象。
- 添加 Linux Steam library 扫描。
- 打包 Linux 版本。
- 通过社区测试验证 Steam Deck Desktop Mode。

## Phase 6：更多游戏

- 添加《怪物猎人：崛起》适配器。
- 等《怪物猎人：荒野》的 Mod 结构稳定后添加适配器。
- 抽取《怪物猎人》系列共享适配工具。
