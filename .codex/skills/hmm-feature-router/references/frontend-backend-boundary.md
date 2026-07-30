# 前端、Tauri 与后端边界

添加 UI、hooks、typed API、DTO、Tauri command、事件、app state 或前端工作流时读取本文件。

主要源文档：

- `docs/ARCHITECTURE.md`
- `docs/APPEARANCE_SYSTEM.md`
- `docs/APPEARANCE_EXTENSION_GUIDE.md`
- `docs/TESTING.md`

## 前端负责

- 布局、组件、CSS、图标、交互细节、loading/error 状态。
- 将 DTO 映射为 UI 状态的 view model。
- Tauri command 的 typed API wrapper。
- 基于后端提供字段和 capability 的搜索/筛选 UI。
- 视觉 polish、响应式、可访问性。

前端不负责：

- 除展示标签以外的文件系统路径。
- 游戏目录校验规则。
- 解压、安装计划、备份、回滚、manifest、冲突语义。
- MHW 专属路径改写或资源编号解析。
- 文件操作安全决策。

## Tauri Command 负责

- 窄用例入口。
- 参数形状校验和 DTO 转换。
- 返回前端可展示的错误 DTO。
- 从 `AppState` 调用 `hmm-app` 服务。

Tauri command 不应：

- 解析 MHW armor slot。
- 为安装拼接 `nativePC` 路径。
- 接受前端传入的导入包 cache path。
- 在没有 task/event 设计时直接执行长时间写入。

## Typed API 模式

优先使用 feature-local typed API：

```text
src/features/<feature>/<feature>Types.ts
src/features/<feature>/<feature>Api.ts
```

`src/shared/api/tauri.ts` 可以放通用 helper 或 re-export 稳定 API，但不要变成巨大的 feature 文件。

前端 DTO 使用 camelCase。Rust DTO 穿过 Tauri 边界时应使用 `#[serde(rename_all = "camelCase")]`。

## UI 样式提示

- 使用 `src/shared/styles/tokens.css` 中的共享 token。
- 局部组件 CSS 使用命名空间。
- 不按 shell/sidebar mode 复制整套页面结构。
- 优先使用项目已有图标库。
- 业务功能页面应偏密集、可扫描、工作流清晰，不做营销式 hero。
