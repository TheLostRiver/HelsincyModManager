# 退出确认与托盘生命周期实现计划

> 本计划记录退出确认弹窗与真实系统托盘路径的实现边界。文档只描述实现方案和验证要求；具体代码以仓库当前实现为准。

**目标：** 点击主窗口关闭按钮时，不直接关闭进程，而是弹出玻璃风格确认 UI；用户可选择“收起至系统托盘”或“完全退出应用程序”，并可记住选择。系统托盘必须是真实 Tauri 托盘，支持恢复窗口和退出程序。

**架构：** Rust/Tauri 负责主窗口关闭拦截、托盘注册、窗口 show / hide / exit 行为，以及 `hmm://window-close-requested` 事件。React 负责弹窗展示、关闭偏好、错误提示和调用窄 command。当前阶段必须诚实说明：托盘模式代表主客户端仍在运行；完全退出后，真正后台 guardian / Windows Scheduled Task 落地前，客户端运行期自动备份不会继续检查。

**技术栈：** Tauri 2.11.2、React 19、TypeScript、`lucide-react`、`localStorage` UI 偏好、HMM CSS token、Rust 单元测试、Node test runner。

## 文件结构

- `src-tauri/src/window_lifecycle_commands.rs`：窗口关闭事件、托盘菜单 id、托盘注册、关闭拦截、`hide_main_window_to_tray`、`exit_app`。
- `src-tauri/src/lib.rs`：注册模块、setup 生命周期、invoke handler。
- `src-tauri/tauri.conf.json`：使用已有 `icons/icon.ico` 与 `icons/icon.png`。
- `src/app/window-lifecycle/windowClosePreference.ts`：关闭行为偏好解析、读取、保存和动作解析。
- `src/app/window-lifecycle/windowLifecycleError.ts`：统一解析 `CommandErrorDto`、`Error` 和字符串错误。
- `src/app/window-lifecycle/windowLifecycleApi.ts`：生命周期 command typed wrapper。
- `src/app/window-lifecycle/useWindowCloseRequest.ts`：监听 `hmm://window-close-requested` 并按偏好执行。
- `src/app/window-lifecycle/WindowCloseDialogHost.tsx`：管理弹窗状态、错误提示和 command 执行。
- `src/app/window-lifecycle/WindowCloseDialog.tsx`：退出确认弹窗 UI。
- `src/app/window-lifecycle/WindowCloseDialog.css`：弹窗局部样式。
- `src/app/window-lifecycle/windowClosePreference.test.mjs`：偏好和错误解析测试。
- `src/app/window-lifecycle/windowLifecycleUi.test.mjs`：UI 源码约束测试。
- `src/app/frame/AppFrame.tsx`：挂载 `WindowCloseDialogHost`。
- `src/features/settings/SettingsPage.tsx` / `SettingsPage.css`：增加窗口行为设置。
- `docs/FRONTEND_BACKEND_CONTRACT.md` / `docs/TESTING.md`：同步通信契约和验证要求。

## 范围护栏

- 不实现真正后台 guardian 或 Windows Scheduled Task。
- 不宣称完全退出后自动备份仍受保护。
- 前端不读取路径、不判断存档备份安全、不执行文件系统规则。
- 不提交 `.planning/`、`.agents/skills/`、临时截图、构建产物、真实存档、真实 Mod、token、cookie 或本地私有路径。

## Task 1：关闭偏好 Helper

**文件：**

- `src/app/window-lifecycle/windowClosePreference.ts`
- `src/app/window-lifecycle/windowClosePreference.test.mjs`

**实现要求：**

- 定义 `WindowClosePreference = "ask" | "tray" | "exit"`。
- 定义 `WindowCloseAction = "show_dialog" | "hide_to_tray" | "exit_app"`。
- 使用 key `hmm.windowCloseBehavior` 保存偏好。
- storage 不可用、内容非法或解析失败时回退 `ask`。
- `saveWindowClosePreference` 必须捕获 `localStorage.setItem` 异常并返回 `false`，避免 UI 状态与持久化状态不一致。

**验证：**

```powershell
cmd /c corepack pnpm run test -- src/app/window-lifecycle/windowClosePreference.test.mjs
```

## Task 2：生命周期错误解析

**文件：**

- `src/app/window-lifecycle/windowLifecycleError.ts`
- `src/app/window-lifecycle/windowClosePreference.test.mjs`

**实现要求：**

- Tauri `invoke` 失败可能是 plain object，例如 `CommandErrorDto { code, message }`。
- `getWindowLifecycleErrorMessage` 应优先读取可用 `message`。
- 保留 `Error`、字符串和兜底文案处理。

**验证：**

- 测试 `CommandErrorDto`、`Error`、字符串和无 message object。

## Task 3：Tauri 窗口生命周期与托盘

**文件：**

- `src-tauri/src/window_lifecycle_commands.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

**实现要求：**

- 事件名固定为 `hmm://window-close-requested`。
- 托盘 id 固定为 `hmm-main-tray`。
- 菜单 id 固定为 `hmm-tray-open` 和 `hmm-tray-exit`。
- 主窗口收到 `CloseRequested` 时调用 `api.prevent_close()`，再 emit 关闭请求事件。
- `hide_main_window_to_tray` 只隐藏主窗口，不执行备份、不读取路径、不修改 Profile。
- `exit_app` 只退出当前 Tauri 主客户端进程。
- 托盘恢复窗口时，`show`、`unminimize`、`set_focus` 失败要写 `tracing::warn!`，方便诊断。
- `TrayIcon` 需要由 app 持有，避免句柄 drop 后托盘消失。

**验证：**

```powershell
cargo test -p hmm-tauri window_lifecycle
cargo check -p hmm-tauri
```

## Task 4：前端生命周期 API 与事件 Hook

**文件：**

- `src/app/window-lifecycle/windowLifecycleApi.ts`
- `src/app/window-lifecycle/useWindowCloseRequest.ts`

**实现要求：**

- `hideMainWindowToTray()` 调用 `hide_main_window_to_tray`。
- `exitApplication()` 调用 `exit_app`。
- hook 监听 `hmm://window-close-requested`。
- 偏好为 `ask` 时显示弹窗；偏好为 `tray` 或 `exit` 时直接调用对应 command。
- hook 内部使用 ref 保存最新 callback，避免因为调用方传入新函数而反复注册 Tauri listener。
- command 失败时展示后端返回的用户可读错误，并回到弹窗。

**验证：**

```powershell
cmd /c corepack pnpm run test -- src/app/window-lifecycle/windowLifecycleUi.test.mjs
```

## Task 5：退出确认弹窗 UI

**文件：**

- `src/app/window-lifecycle/WindowCloseDialogHost.tsx`
- `src/app/window-lifecycle/WindowCloseDialog.tsx`
- `src/app/window-lifecycle/WindowCloseDialog.css`
- `src/app/frame/AppFrame.tsx`

**实现要求：**

- 弹窗提供“收起至系统托盘”和“完全退出应用程序”两个明确操作。
- 支持“记住我的选择，下次直接执行”。
- 保存“记住选择”失败时，不执行隐藏/退出动作，保留弹窗并提示用户。
- 支持 `Escape` 取消。
- `aria-modal="true"` 的 dialog 必须 trap focus，Tab / Shift+Tab 不能逃到背景页面。
- 打开时聚焦弹窗容器；延迟 focus 的 timer 必须在 cleanup 中清理。
- 小窗口下按钮和文案不能溢出或遮挡。

**验证：**

```powershell
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

## Task 6：设置页窗口行为入口

**文件：**

- `src/features/settings/SettingsPage.tsx`
- `src/features/settings/SettingsPage.css`

**实现要求：**

- 设置项为“关闭主窗口时”。
- 选项为“每次询问”“收起至托盘”“退出应用”。
- 保存偏好成功后才更新 UI 选中态。
- 保存失败时显示 `role="alert"` 提示，不让 UI 假装已经保存。
- 文案必须明确：真正后台守护尚未落地；选择退出应用后，客户端运行期自动备份不会继续检查。

## Task 7：契约与测试文档

**文件：**

- `docs/FRONTEND_BACKEND_CONTRACT.md`
- `docs/TESTING.md`

**实现要求：**

- 记录 `hmm://window-close-requested`。
- 记录 `hide_main_window_to_tray` 和 `exit_app` 的边界。
- 记录托盘模式与完全退出对自动备份的不同影响。
- 记录窗口关闭与托盘生命周期切片的最小验证命令。

## Task 8：运行时和视觉 Smoke

**手动检查：**

1. 点击原生关闭按钮，弹窗出现。
2. 按 `Esc`，弹窗关闭且主窗口仍可见。
3. 再次关闭，选择“收起至系统托盘”，主窗口隐藏。
4. 点击托盘图标或菜单“打开 Helsincy”，窗口恢复并聚焦。
5. 勾选“记住我的选择”后选择“收起至系统托盘”，下次关闭直接隐藏。
6. 在设置页把“关闭主窗口时”改回“每次询问”。
7. 再次关闭，弹窗重新出现。
8. 选择“完全退出应用程序”，Tauri 进程退出。

**推荐视口：**

- `1440x900`
- `1366x768`
- `1280x800`
- `960x640`

## 提交计划

推荐按范围提交：

1. `test: 覆盖窗口关闭偏好与错误解析`
2. `feat: 添加窗口关闭与托盘生命周期命令`
3. `feat: 添加退出确认弹窗`
4. `feat: 添加窗口关闭行为设置`
5. `docs: 记录窗口关闭与托盘生命周期契约`

实际 PR 可以在 review 修复阶段追加单独提交，例如：

```text
fix: 处理退出生命周期 review 反馈
```

## 自审清单

- 关闭事件、托盘 id、菜单 id 和 command 名称稳定。
- 前端只调用窄 command，不读取路径、不执行文件系统逻辑。
- 完全退出文案不承诺后台自动备份继续运行。
- 保存偏好失败时 UI 不显示错误状态之外的假成功。
- `CommandErrorDto` 的 `message` 不被吞掉。
- modal 键盘焦点不会逃到背景页面。
- 托盘恢复失败有最小诊断日志。
- 验证命令真实执行，并在 PR 或最终回复中记录结果。