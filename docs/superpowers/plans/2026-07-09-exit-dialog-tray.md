# Exit Dialog Tray Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a real close-window experience: a glass-style exit dialog based on the user-owned HTML demo, with a working system tray path and a real quit path.

**Architecture:** Rust/Tauri owns window close interception, tray registration, show/hide/exit behavior, and a narrow close-request event. React owns presentation, user preference, and calls narrow lifecycle commands. Current automatic backup truth stays honest: tray mode keeps the main client alive; full exit stops client-runtime automatic backup checks until the real guardian/Scheduled Task lands.

**Tech Stack:** Tauri 2.11.2, React 19, TypeScript, lucide-react, localStorage-backed UI preference, HMM CSS tokens, Rust unit tests, Node test runner.

---

## File Structure

- Create `src-tauri/src/window_lifecycle_commands.rs`: event constants, tray menu ids, tray registration, close interception, `hide_main_window_to_tray`, `exit_app`.
- Modify `src-tauri/src/lib.rs`: module import, setup registration, invoke handlers.
- Modify `src-tauri/tauri.conf.json`: use existing `icons/icon.ico` and `icons/icon.png` for default/tray icon support.
- Create `src/app/window-lifecycle/windowClosePreference.ts`: parse/load/save/resolve close behavior.
- Create `src/app/window-lifecycle/windowClosePreference.test.mjs`: focused preference tests.
- Create `src/app/window-lifecycle/windowLifecycleApi.ts`: typed wrappers for lifecycle commands and event name.
- Create `src/app/window-lifecycle/useWindowCloseRequest.ts`: listens for `hmm://window-close-requested`.
- Create `src/app/window-lifecycle/WindowCloseDialogHost.tsx`: owns dialog state and command execution.
- Create `src/app/window-lifecycle/WindowCloseDialog.tsx`: glass dialog adapted from the user-provided local HTML demo; the demo file itself is not part of the repository.
- Create `src/app/window-lifecycle/WindowCloseDialog.css`: scoped CSS, no external font, no full-screen decorative background.
- Modify `src/app/frame/AppFrame.tsx`: mount one dialog host.
- Modify `src/features/settings/SettingsPage.tsx` and `.css`: add a window close behavior setting so remembered choice can be changed.
- Modify `docs/FRONTEND_BACKEND_CONTRACT.md` and `docs/TESTING.md`: document the event, commands, and verification.

## Scope Guard

- This plan does not implement the real background guardian or Windows Scheduled Task.
- The UI must not claim full exit is protected by background automatic backup.
- Frontend never decides save-backup safety. It only displays copy and calls narrow lifecycle commands.
- Do not stage `.planning/`, `.superpowers/`, `.agents/skills/`, temp previews, screenshots, build output, or the pre-existing unrelated `TODO.md` sync unless the PR explicitly includes them.

---

### Task 1: Close Preference Helper

**Files:**
- Create: `src/app/window-lifecycle/windowClosePreference.ts`
- Create: `src/app/window-lifecycle/windowClosePreference.test.mjs`

- [ ] **Step 1: Write failing tests**

Create `src/app/window-lifecycle/windowClosePreference.test.mjs`:

```js
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  CLOSE_BEHAVIOR_STORAGE_KEY,
  loadWindowClosePreference,
  resolveWindowCloseAction,
  saveWindowClosePreference,
} from "./windowClosePreference.ts";

function createStorage(initial = null) {
  const store = new Map();
  if (initial !== null) store.set(CLOSE_BEHAVIOR_STORAGE_KEY, initial);
  return {
    getItem: (key) => (store.has(key) ? store.get(key) : null),
    setItem: (key, value) => store.set(key, value),
  };
}

test("loads ask when storage is unavailable or invalid", () => {
  assert.equal(loadWindowClosePreference(undefined), "ask");
  assert.equal(loadWindowClosePreference(createStorage(JSON.stringify("bad"))), "ask");
  assert.equal(loadWindowClosePreference(createStorage("not-json")), "ask");
});

test("saves and loads stable close behavior values", () => {
  const storage = createStorage();
  saveWindowClosePreference(storage, "tray");
  assert.equal(loadWindowClosePreference(storage), "tray");
  saveWindowClosePreference(storage, "exit");
  assert.equal(loadWindowClosePreference(storage), "exit");
});

test("resolves stored preferences to close actions", () => {
  assert.equal(resolveWindowCloseAction("ask"), "show_dialog");
  assert.equal(resolveWindowCloseAction("tray"), "hide_to_tray");
  assert.equal(resolveWindowCloseAction("exit"), "exit_app");
});
```

- [ ] **Step 2: Verify red**

Run: `cmd /c corepack pnpm run test -- src/app/window-lifecycle/windowClosePreference.test.mjs`

Expected: fails because `windowClosePreference.ts` does not exist.

- [ ] **Step 3: Implement helper**

Create `src/app/window-lifecycle/windowClosePreference.ts`:

```ts
export const CLOSE_BEHAVIOR_STORAGE_KEY = "hmm.windowCloseBehavior";
export type WindowClosePreference = "ask" | "tray" | "exit";
export type WindowCloseAction = "show_dialog" | "hide_to_tray" | "exit_app";

type PreferenceStorage = Pick<Storage, "getItem" | "setItem"> | undefined;
const VALID_PREFERENCES = new Set<WindowClosePreference>(["ask", "tray", "exit"]);

export function parseWindowClosePreference(value: unknown): WindowClosePreference {
  return typeof value === "string" && VALID_PREFERENCES.has(value as WindowClosePreference)
    ? (value as WindowClosePreference)
    : "ask";
}

export function loadWindowClosePreference(storage: PreferenceStorage = window.localStorage): WindowClosePreference {
  if (!storage) return "ask";
  try {
    return parseWindowClosePreference(JSON.parse(storage.getItem(CLOSE_BEHAVIOR_STORAGE_KEY) ?? "null"));
  } catch {
    return "ask";
  }
}

export function saveWindowClosePreference(
  storage: PreferenceStorage = window.localStorage,
  preference: WindowClosePreference,
): void {
  if (!storage) return;
  storage.setItem(CLOSE_BEHAVIOR_STORAGE_KEY, JSON.stringify(parseWindowClosePreference(preference)));
}

export function resolveWindowCloseAction(preference: WindowClosePreference): WindowCloseAction {
  if (preference === "tray") return "hide_to_tray";
  if (preference === "exit") return "exit_app";
  return "show_dialog";
}
```

- [ ] **Step 4: Verify green**

Run: `cmd /c corepack pnpm run test -- src/app/window-lifecycle/windowClosePreference.test.mjs`

Expected: pass.

---

### Task 2: Tauri Window Lifecycle Commands And Tray

**Files:**
- Create: `src-tauri/src/window_lifecycle_commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Add Rust module with stable ids, tray registration, commands**

Create `src-tauri/src/window_lifecycle_commands.rs`:

```rust
use crate::dto::CommandErrorDto;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, Window, WindowEvent};

pub const WINDOW_CLOSE_REQUESTED_EVENT: &str = "hmm://window-close-requested";
const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "hmm-main-tray";
const MENU_OPEN_ID: &str = "hmm-tray-open";
const MENU_EXIT_ID: &str = "hmm-tray-exit";

fn window_lifecycle_error(code: &'static str, message: impl Into<String>) -> CommandErrorDto {
    CommandErrorDto { code: code.to_owned(), message: message.into() }
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn register_window_lifecycle(app: &mut App) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, MENU_OPEN_ID, "打开 Helsincy", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let exit_item = MenuItem::with_id(app, MENU_EXIT_ID, "退出程序", true, None::<&str>)?;
    let tray_menu = Menu::with_items(app, &[&open_item, &separator, &exit_item])?;

    let mut tray_builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Helsincy Mod Manager")
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_OPEN_ID => show_main_window(app),
            MENU_EXIT_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        tray_builder = tray_builder.icon(icon);
    }
    tray_builder.build(app)?;

    app.on_window_event(|window, event| {
        if window.label() != MAIN_WINDOW_LABEL { return; }
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window.emit(WINDOW_CLOSE_REQUESTED_EVENT, ());
        }
    });

    Ok(())
}

#[tauri::command]
pub fn hide_main_window_to_tray(window: Window) -> Result<(), CommandErrorDto> {
    window.hide().map_err(|error| window_lifecycle_error("window_hide_failed", error.to_string()))
}

#[tauri::command]
pub fn exit_app(app: AppHandle) -> Result<(), CommandErrorDto> {
    app.exit(0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_lifecycle_event_and_menu_ids_are_stable() {
        assert_eq!(WINDOW_CLOSE_REQUESTED_EVENT, "hmm://window-close-requested");
        assert_eq!(MAIN_WINDOW_LABEL, "main");
        assert_eq!(TRAY_ID, "hmm-main-tray");
        assert_eq!(MENU_OPEN_ID, "hmm-tray-open");
        assert_eq!(MENU_EXIT_ID, "hmm-tray-exit");
    }

    #[test]
    fn window_lifecycle_error_uses_stable_code_without_paths() {
        let dto = window_lifecycle_error("window_hide_failed", "hide failed");
        assert_eq!(dto.code, "window_hide_failed");
        assert_eq!(dto.message, "hide failed");
        assert!(!dto.message.contains("C:/"));
    }
}
```

- [ ] **Step 2: Wire in `src-tauri/src/lib.rs`**

Add module:

```rust
mod window_lifecycle_commands;
```

Add import:

```rust
use window_lifecycle_commands::{exit_app, hide_main_window_to_tray, register_window_lifecycle};
```

Inside setup, after `app.manage(state);`, add:

```rust
            register_window_lifecycle(app)?;
```

Add invoke handlers:

```rust
            hide_main_window_to_tray,
            exit_app,
```

- [ ] **Step 3: Configure icons**

Change `src-tauri/tauri.conf.json`:

```json
"icon": ["icons/icon.ico", "icons/icon.png"]
```

- [ ] **Step 4: Verify**

Run:

```powershell
cargo test -p hmm-tauri window_lifecycle
cargo check -p hmm-tauri
```

Expected: both pass.

---

### Task 3: Frontend Lifecycle API And Host

**Files:**
- Create: `src/app/window-lifecycle/windowLifecycleApi.ts`
- Create: `src/app/window-lifecycle/useWindowCloseRequest.ts`
- Create: `src/app/window-lifecycle/WindowCloseDialogHost.tsx`
- Modify: `src/app/frame/AppFrame.tsx`

- [ ] **Step 1: Add typed API wrappers**

Create `src/app/window-lifecycle/windowLifecycleApi.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";

export const WINDOW_CLOSE_REQUESTED_EVENT = "hmm://window-close-requested";

export function hideMainWindowToTray(): Promise<void> {
  return invoke<void>("hide_main_window_to_tray");
}

export function exitApplication(): Promise<void> {
  return invoke<void>("exit_app");
}
```

- [ ] **Step 2: Add close-event hook**

Create `src/app/window-lifecycle/useWindowCloseRequest.ts`:

```ts
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { hideMainWindowToTray, exitApplication, WINDOW_CLOSE_REQUESTED_EVENT } from "./windowLifecycleApi";
import { loadWindowClosePreference, resolveWindowCloseAction } from "./windowClosePreference";

type UseWindowCloseRequestOptions = {
  onShowDialog: () => void;
  onError: (message: string) => void;
};

export function useWindowCloseRequest({ onShowDialog, onError }: UseWindowCloseRequestOptions) {
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void listen(WINDOW_CLOSE_REQUESTED_EVENT, () => {
      const action = resolveWindowCloseAction(loadWindowClosePreference());
      if (action === "show_dialog") {
        onShowDialog();
        return;
      }
      const command = action === "hide_to_tray" ? hideMainWindowToTray : exitApplication;
      void command().catch((error: unknown) => {
        onError(error instanceof Error ? error.message : "窗口关闭操作失败");
        onShowDialog();
      });
    }).then((dispose) => {
      if (disposed) dispose();
      else unlisten = dispose;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [onError, onShowDialog]);
}
```

- [ ] **Step 3: Add host**

Create `src/app/window-lifecycle/WindowCloseDialogHost.tsx`:

```tsx
import { useCallback, useState } from "react";
import { WindowCloseDialog } from "./WindowCloseDialog";
import { exitApplication, hideMainWindowToTray } from "./windowLifecycleApi";
import { saveWindowClosePreference, type WindowClosePreference } from "./windowClosePreference";
import { useWindowCloseRequest } from "./useWindowCloseRequest";

export function WindowCloseDialogHost() {
  const [open, setOpen] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const showDialog = useCallback(() => {
    setErrorMessage(null);
    setOpen(true);
  }, []);

  const handleError = useCallback((message: string) => setErrorMessage(message), []);
  useWindowCloseRequest({ onShowDialog: showDialog, onError: handleError });

  const runAction = useCallback(async (action: WindowClosePreference, remember: boolean) => {
    if (remember) saveWindowClosePreference(undefined, action);
    if (action === "tray") {
      await hideMainWindowToTray();
      setOpen(false);
      return;
    }
    await exitApplication();
  }, []);

  return <WindowCloseDialog open={open} errorMessage={errorMessage} onCancel={() => setOpen(false)} onConfirm={runAction} />;
}
```

- [ ] **Step 4: Mount host in `AppFrame.tsx`**

Import:

```tsx
import { WindowCloseDialogHost } from "../window-lifecycle/WindowCloseDialogHost";
```

Render after `.app-surface` inside `.app-shell`:

```tsx
      <WindowCloseDialogHost />
```

- [ ] **Step 5: Verify expected missing component**

Run: `cmd /c corepack pnpm run typecheck`

Expected: fails until Task 4 creates `WindowCloseDialog`.

---

### Task 4: Glass Exit Dialog UI

**Files:**
- Create: `src/app/window-lifecycle/WindowCloseDialog.tsx`
- Create: `src/app/window-lifecycle/WindowCloseDialog.css`

- [ ] **Step 1: Create component**

Create `src/app/window-lifecycle/WindowCloseDialog.tsx`:

```tsx
import { Check, LoaderCircle, Minimize2, Power, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { WindowClosePreference } from "./windowClosePreference";
import "./WindowCloseDialog.css";

type WindowCloseDialogProps = {
  open: boolean;
  errorMessage: string | null;
  onCancel: () => void;
  onConfirm: (action: WindowClosePreference, remember: boolean) => Promise<void>;
};

type ExecutingAction = "tray" | "exit" | null;

export function WindowCloseDialog({ open, errorMessage, onCancel, onConfirm }: WindowCloseDialogProps) {
  const [remember, setRemember] = useState(false);
  const [executing, setExecuting] = useState<ExecutingAction>(null);
  const [successText, setSuccessText] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    setRemember(false);
    setExecuting(null);
    setSuccessText(null);
    window.setTimeout(() => dialogRef.current?.focus(), 0);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !executing) onCancel();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [executing, onCancel, open]);

  if (!open) return null;

  const execute = async (action: "tray" | "exit") => {
    setExecuting(action);
    setSuccessText(action === "tray" ? "已收起至系统托盘" : "正在退出应用");
    try {
      await new Promise((resolve) => window.setTimeout(resolve, action === "exit" ? 420 : 720));
      await onConfirm(action, remember);
    } catch {
      setExecuting(null);
      setSuccessText(null);
    }
  };

  return (
    <div className="window-close-overlay" onMouseDown={(event) => event.target === event.currentTarget && !executing && onCancel()}>
      <div ref={dialogRef} className="window-close-dialog" role="dialog" aria-modal="true" aria-labelledby="window-close-title" tabIndex={-1}>
        <button className="window-close-dialog__close" type="button" onClick={onCancel} disabled={Boolean(executing)} aria-label="取消关闭">
          <X size={15} strokeWidth={2.2} />
        </button>
        <header className="window-close-dialog__header">
          <h2 id="window-close-title">准备退出 Helsincy？</h2>
          <p>请选择关闭主窗口时的操作。你也可以在设置里随时改回每次询问。</p>
        </header>
        {errorMessage ? <p className="window-close-dialog__error">{errorMessage}</p> : null}
        <div className="window-close-dialog__options">
          <button className="window-close-option is-tray" type="button" onClick={() => void execute("tray")} disabled={Boolean(executing)}>
            <span className="window-close-option__icon" aria-hidden="true"><Minimize2 size={24} /></span>
            <span className="window-close-option__copy"><strong>收起至系统托盘</strong><span>应用将在后台持续运行，自动备份仍会在客户端运行期间检查。</span></span>
            {executing === "tray" ? <LoaderCircle className="window-close-option__spinner" size={22} /> : null}
          </button>
          <button className="window-close-option is-exit" type="button" onClick={() => void execute("exit")} disabled={Boolean(executing)}>
            <span className="window-close-option__icon" aria-hidden="true"><Power size={24} /></span>
            <span className="window-close-option__copy"><strong>完全退出应用程序</strong><span>关闭主客户端。后台守护落地前，自动备份不会继续检查。</span></span>
            {executing === "exit" ? <LoaderCircle className="window-close-option__spinner" size={22} /> : null}
          </button>
        </div>
        <footer className="window-close-dialog__footer">
          <label className="window-close-dialog__remember">
            <input type="checkbox" checked={remember} onChange={(event) => setRemember(event.target.checked)} disabled={Boolean(executing)} />
            <span className="window-close-dialog__checkbox" aria-hidden="true"><Check size={12} strokeWidth={2.6} /></span>
            <span>记住我的选择，下次直接执行</span>
          </label>
          <button className="window-close-dialog__cancel" type="button" onClick={onCancel} disabled={Boolean(executing)}>暂不退出</button>
        </footer>
        {successText ? <div className="window-close-dialog__success" aria-live="polite"><span><Check size={30} strokeWidth={3} /></span><strong>{successText}</strong></div> : null}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Create CSS**

Create `src/app/window-lifecycle/WindowCloseDialog.css` with scoped selectors copied/adapted from the user-owned demo. Required constraints:

```css
.window-close-overlay { position: fixed; inset: 0; z-index: 200; display: grid; place-items: center; padding: 24px; background: rgba(4, 6, 12, 0.48); backdrop-filter: blur(8px); }
.window-close-dialog { position: relative; width: min(480px, 100%); padding: 36px; color: #f8fafc; background: rgba(15, 22, 38, 0.78); border: 1px solid rgba(255,255,255,0.08); border-top-color: rgba(255,255,255,0.16); border-radius: 28px; box-shadow: 0 24px 70px rgba(0,0,0,0.62), inset 0 1px 1px rgba(255,255,255,0.05); backdrop-filter: blur(28px) saturate(180%); outline: none; }
.window-close-dialog__close { position: absolute; top: 24px; right: 24px; display: grid; place-items: center; width: 32px; height: 32px; color: #94a3b8; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.06); border-radius: 999px; cursor: pointer; }
.window-close-dialog__header { display: grid; gap: 8px; margin-bottom: 24px; padding-right: 34px; }
.window-close-dialog__header h2 { margin: 0; font-size: 24px; font-weight: 700; letter-spacing: 0; }
.window-close-dialog__header p, .window-close-dialog__error { margin: 0; color: #94a3b8; font-size: 14px; line-height: 1.5; }
.window-close-dialog__options { display: grid; gap: 16px; margin-bottom: 26px; }
.window-close-option { position: relative; display: grid; grid-template-columns: auto minmax(0, 1fr) auto; gap: 20px; align-items: center; min-height: 94px; padding: 20px; text-align: left; color: inherit; background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.05); border-radius: 20px; cursor: pointer; overflow: hidden; }
.window-close-option__icon { display: grid; place-items: center; width: 52px; height: 52px; color: #fff; border-radius: 14px; }
.window-close-option.is-tray .window-close-option__icon { background: linear-gradient(135deg, #3b82f6, #1d4ed8); box-shadow: 0 6px 16px rgba(59,130,246,0.25); }
.window-close-option.is-exit .window-close-option__icon { background: linear-gradient(135deg, #ef4444, #b91c1c); box-shadow: 0 6px 16px rgba(239,68,68,0.25); }
.window-close-option__copy { display: grid; gap: 4px; min-width: 0; }
.window-close-option__copy strong { color: #f1f5f9; font-size: 16px; font-weight: 700; }
.window-close-option__copy span { color: #94a3b8; font-size: 13px; line-height: 1.42; }
.window-close-dialog__footer { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
.window-close-dialog__remember { display: inline-flex; align-items: center; gap: 10px; min-width: 0; color: #94a3b8; font-size: 13.5px; cursor: pointer; }
.window-close-dialog__remember input { position: absolute; opacity: 0; }
.window-close-dialog__checkbox { display: grid; place-items: center; width: 18px; height: 18px; color: transparent; background: rgba(255,255,255,0.03); border: 1.5px solid rgba(255,255,255,0.15); border-radius: 6px; }
.window-close-dialog__remember input:checked + .window-close-dialog__checkbox { color: white; background: #6366f1; border-color: #6366f1; }
.window-close-dialog__cancel { min-width: max-content; padding: 8px 14px; color: #94a3b8; background: transparent; border: 0; border-radius: 10px; cursor: pointer; font: inherit; font-size: 13.5px; font-weight: 600; }
.window-close-dialog__success { position: absolute; inset: 0; z-index: 3; display: grid; place-items: center; align-content: center; gap: 16px; color: #f1f5f9; background: #090d16; border-radius: inherit; }
@media (max-width: 560px) { .window-close-dialog { padding: 28px 20px; border-radius: 22px; } .window-close-option { grid-template-columns: auto minmax(0, 1fr); } .window-close-dialog__footer { align-items: flex-start; flex-direction: column; } }
```

Then add hover/focus/reduced-motion refinements from the demo without adding external fonts, page-level blobs, or unscoped selectors.

- [ ] **Step 3: Verify frontend**

Run:

```powershell
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
```

Expected: all pass.

---

### Task 5: Settings Entry For Remembered Close Behavior

**Files:**
- Modify: `src/features/settings/SettingsPage.tsx`
- Modify: `src/features/settings/SettingsPage.css`

- [ ] **Step 1: Add imports and state**

Modify `SettingsPage.tsx` imports:

```tsx
import { Bell, Check, Database, FileArchive, MonitorCog, RotateCcw, Save, ShieldCheck, SlidersHorizontal } from "lucide-react";
import { loadWindowClosePreference, saveWindowClosePreference, type WindowClosePreference } from "../../app/window-lifecycle/windowClosePreference";
```

Inside `SettingsPage` add:

```tsx
  const [windowClosePreference, setWindowClosePreference] = useState<WindowClosePreference>(() =>
    typeof window === "undefined" ? "ask" : loadWindowClosePreference(),
  );

  const updateWindowClosePreference = (value: WindowClosePreference) => {
    setWindowClosePreference(value);
    saveWindowClosePreference(undefined, value);
  };
```

- [ ] **Step 2: Add settings section**

After the interface preference section, add:

```tsx
        <SettingsSection
          title="窗口行为"
          description="控制点击窗口关闭按钮时的默认动作；这不会改变后台守护是否已启用。"
          icon={MonitorCog}
        >
          <ChoiceGroup
            label="关闭主窗口时"
            value={windowClosePreference}
            options={[
              { value: "ask", label: "每次询问" },
              { value: "tray", label: "收起至托盘" },
              { value: "exit", label: "退出应用" },
            ]}
            onChange={updateWindowClosePreference}
          />
          <div className="settings-callout settings-callout--neutral" role="note">
            <Bell size={16} strokeWidth={2.1} />
            <span>当前真正后台守护尚未落地；选择退出应用后，客户端运行期自动备份不会继续检查。</span>
          </div>
        </SettingsSection>
```

- [ ] **Step 3: Add neutral callout style**

Append to `SettingsPage.css`:

```css
.settings-callout--neutral { color: var(--color-neutral-text); background: var(--color-neutral-bg); border-left-color: var(--color-neutral-dot); }
```

- [ ] **Step 4: Verify**

Run:

```powershell
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run test -- src/app/window-lifecycle/windowClosePreference.test.mjs
```

Expected: all pass.

---

### Task 6: Contract And Testing Docs

**Files:**
- Modify: `docs/FRONTEND_BACKEND_CONTRACT.md`
- Modify: `docs/TESTING.md`

- [ ] **Step 1: Update frontend/backend contract**

Add this section to `docs/FRONTEND_BACKEND_CONTRACT.md`:

```markdown
### 窗口关闭与托盘生命周期

- `hmm://window-close-requested` 由 Tauri 后端在主窗口收到关闭请求时发出；后端会先阻止默认关闭，前端必须显示关闭选择或按已保存偏好调用窄命令。
- `hide_main_window_to_tray` 只隐藏当前主窗口，不执行备份、不修改 Profile、不读取路径。
- `exit_app` 只退出当前 Tauri 主客户端进程，不声明后台守护已接管。
- 当前真正后台守护 / Windows Scheduled Task 尚未落地；前端文案必须区分“托盘后台运行”与“完全退出应用”。完全退出后，客户端运行期自动备份不会继续检查。
- 前端不得通过宽泛 window/filesystem API 重建生命周期逻辑；只调用本节列出的窄命令。
```

- [ ] **Step 2: Update testing guide**

Add this to `docs/TESTING.md`:

```markdown
窗口关闭与托盘生命周期切片至少运行：

- `cmd /c corepack pnpm run test -- src/app/window-lifecycle/windowClosePreference.test.mjs`
- `cmd /c corepack pnpm run typecheck`
- `cmd /c corepack pnpm run lint`
- `cmd /c corepack pnpm run build`
- `cargo test -p hmm-tauri window_lifecycle`
- `cargo check -p hmm-tauri`

可视化检查需要覆盖：关闭按钮弹窗、收起至托盘后从托盘恢复、完全退出、记住选择、设置页改回每次询问。当前阶段不得把完全退出描述为后台自动备份仍受保护。
```

- [ ] **Step 3: Verify docs**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-doc-links.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\check-secrets.ps1
```

Expected: both pass.

---

### Task 7: Visual And Runtime Verification

**Files:**
- No source changes unless verification finds a defect.

- [ ] **Step 1: Full verification**

Run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

Expected: `Verification passed.`

- [ ] **Step 2: Tauri smoke test**

Run:

```powershell
cmd /c corepack pnpm tauri dev
```

Expected: app opens without command errors.

- [ ] **Step 3: Manual smoke checklist**

1. Click the native close button.
2. Confirm the glass close dialog appears centered over the app.
3. Press `Esc`; dialog closes and the app remains visible.
4. Click close again, choose `收起至系统托盘`; the main window hides.
5. Click tray icon or `打开 Helsincy`; the window shows and focuses.
6. Click close again, check `记住我的选择`, choose `收起至系统托盘`.
7. Reopen from tray, click close again; it hides directly without dialog.
8. Open Settings, set `关闭主窗口时` to `每次询问`.
9. Click close again; dialog appears again.
10. Choose `完全退出应用程序`; Tauri process exits.

- [ ] **Step 4: Visual smoke widths**

Check:

```text
1440x900
1366x768
1280x800
960x640
```

Expected: no text overlap, no clipped controls, close button reachable, option cards readable.

- [ ] **Step 5: Hygiene**

Run:

```powershell
git status --short --branch
git diff --stat
```

Expected: only intended files are changed. Keep the pre-existing `TODO.md` sync separate unless the PR explicitly includes it.

---

## Commit Plan

Use small commits when executing:

1. `test: 覆盖窗口关闭偏好解析`
2. `feat: 添加窗口关闭与托盘生命周期命令`
3. `feat: 添加关闭应用确认弹窗`
4. `feat: 添加窗口关闭行为偏好`
5. `docs: 记录窗口关闭与托盘生命周期合同`

## Self-Review

- Spec coverage: covers selected B scope: close dialog, tray hide/show, true exit, remembered choice, settings recovery path, honest automatic-backup copy, contracts, and verification.
- Placeholder scan: no open placeholder markers are required for implementation; event names, storage keys, file paths, commands, and verification commands are concrete.
- Type consistency: `WindowClosePreference` is `"ask" | "tray" | "exit"`; resolved actions are `"show_dialog" | "hide_to_tray" | "exit_app"`; Rust event is `hmm://window-close-requested`; commands are `hide_main_window_to_tray` and `exit_app`.
- Scope guard: this plan does not implement guardian/headless worker/Windows Scheduled Task. It only makes tray mode real and prevents full-exit copy from claiming background protection.
