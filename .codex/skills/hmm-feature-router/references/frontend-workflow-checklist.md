# Frontend Workflow Checklist

用于 HMM React UI、CSS、appearance、page 和 typed API 工作。

## 结构

- 文件位于预期边界：`src/app`、`src/features` 或 `src/shared`。
- Component 只拥有 presentation 和 local UI state。
- Feature state 和 view models 消费 typed DTOs，不重建 backend rules。
- 新文件不要把一个 component/page 变成 UI/state/API/rules 混合文件。

## API 边界

- Feature command calls 有 feature-local wrapper。
- 已检查 `docs/FRONTEND_BACKEND_CONTRACT.md` 中 command names、DTO shape、errors、task events、phase codes、thumbnail/custom protocol rules 和所需文档更新。
- Shared Tauri helper 只提供通用 invoke mechanics。
- DTO field names 匹配实际 camelCase contract。
- UI 不提交最终 install paths、cache paths、thumbnail disk paths 或 retargeted `nativePC` paths。
- `message` 文本仅用于展示；分支逻辑使用稳定 code/kind/status/phase values。

## Appearance 和 Layout

- 使用 semantic CSS variables/tokens，不使用大范围 hard-coded palettes。
- Component CSS 有本地 namespace。
- Shell/sidebar/theme/density variants 不复制业务页面。
- Navigation source 保持单一。
- 文本在相关宽度下适配 controls/cards。
- Icon-only controls 有 accessible names 或 tooltips。
- 触及 shell/sidebar/Dashboard 时，visual baselines 覆盖 `1440x900`、`1366x768` 和 `1280x800`。

## 安全

- Frontend 中不得有 filesystem safety、install、backup、rollback、manifest、conflict、MHW path 或 save logic。
- 除非 contract 明确允许，不使用 `convertFileSrc`、asset protocol、raw cache path 或 arbitrary local path read。
- Fixtures、logs、docs 或 screenshots 中不得出现真实 player paths、Steam IDs、Mod contents、save contents、tokens、cookies 或 API keys。

## 验证

- 开发期间运行与改动匹配的 typecheck、lint、聚焦 frontend test 或 browser smoke，不把全部命令当成
  每个 commit 的固定套餐。
- Bundling、route registration、asset 或 build config 变化时补 production build。
- 首次 PR ready 前按 router 风险分级决定是否由完整 `verify.ps1` 覆盖全套 frontend checks。
- App shell/sidebar/Dashboard work 已运行 `scripts/check-frontend-boundaries.ps1`。
- Visual changes 已在相关 states/viewports 检查，并记录 findings。
- 跨 Tauri 边界时同时读取 `tauri-command-checklist.md`。
