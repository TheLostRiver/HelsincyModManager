import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "../../..");

function readProjectFile(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

test("window close hook keeps one Tauri listener while reading latest callbacks", () => {
  const source = readProjectFile("src/app/window-lifecycle/useWindowCloseRequest.ts");

  assert.match(source, /useRef/);
  assert.match(source, /callbacksRef\.current\s*=\s*\{ onShowDialog, onError \}/);
  assert.match(source, /callbacksRef\.current\.onShowDialog\(\{ kind: "normal" \}\)/);
  assert.match(source, /callbacksRef\.current\.onError\(getWindowLifecycleErrorMessage\(error\)\)/);
  assert.match(source, /useEffect\(\(\) => \{[\s\S]*?\}, \[\]\);/);
});

test("ordinary and override exits use explicit flags and the structured exit guard", () => {
  const api = readProjectFile("src/app/window-lifecycle/windowLifecycleApi.ts");
  const hook = readProjectFile("src/app/window-lifecycle/useWindowCloseRequest.ts");
  const host = readProjectFile("src/app/window-lifecycle/WindowCloseDialogHost.tsx");

  assert.match(api, /exitApplication\(overrideUnprotected = false\)/);
  assert.match(api, /request:\s*\{ overrideUnprotected \}/);
  assert.match(api, /invoke<AppExitGuardDto>\("get_app_exit_guard"\)/);
  assert.match(hook, /getAppExitGuard\(\)/);
  assert.match(hook, /exitApplication\(false\)/);
  assert.match(hook, /exit_confirmation_required/);
  assert.match(hook, /MAX_ORDINARY_EXIT_ATTEMPTS/);
  assert.match(hook, /return "status_unavailable"/);
  assert.doesNotMatch(hook, /\.message/);
  assert.match(host, /exitApplication\(true\)/);
});

test("window close host carries normal and unsafe dialog modes", () => {
  const host = readProjectFile("src/app/window-lifecycle/WindowCloseDialogHost.tsx");
  const hook = readProjectFile("src/app/window-lifecycle/useWindowCloseRequest.ts");

  assert.match(host, /WindowCloseDialogMode/);
  assert.match(hook, /kind: "normal"/);
  assert.match(host, /kind: "unsafe"/);
  assert.match(host, /reason/);
  assert.match(host, /mode=\{mode\}/);
});

test("a guarded exit race restores the preference written by the normal dialog", () => {
  const host = readProjectFile("src/app/window-lifecycle/WindowCloseDialogHost.tsx");

  assert.match(host, /loadWindowClosePreference\(\)/);
  assert.match(host, /preferenceSaved/);
  assert.match(host, /if \(reason[\s\S]*?saveWindowClosePreference\(undefined, previousPreference\)/);
});

test("window close dialog traps focus and submits the safe tray action with Enter", () => {
  const source = readProjectFile("src/app/window-lifecycle/WindowCloseDialog.tsx");

  assert.match(source, /FOCUSABLE_SELECTOR/);
  assert.match(source, /getFocusableDialogElements/);
  assert.match(source, /event\.key === "Enter"/);
  assert.match(source, /event\.preventDefault\(\);[\s\S]*?void execute\("tray"\)/);
  assert.match(source, /phase !== "closing"/);
  assert.match(source, /event\.key !== "Tab"/);
  assert.match(source, /event\.shiftKey/);
  assert.match(source, /firstFocusable/);
  assert.match(source, /lastFocusable/);
  assert.match(source, /clearTimeout\(focusTimer\)/);
});

test("normal and unsafe dialogs default focus to tray and unsafe cannot persist a preference", () => {
  const source = readProjectFile("src/app/window-lifecycle/WindowCloseDialog.tsx");

  assert.match(source, /renderedMode\.kind === "unsafe"/);
  assert.match(source, /role=\{renderedMode\.kind === "unsafe" \? "alertdialog" : "dialog"\}/);
  assert.match(source, /trayButtonRef/);
  assert.match(source, /setTimeout\(\(\) => trayButtonRef\.current\?\.focus\(\), 0\)/);
  assert.match(source, /renderedMode\.kind === "normal"[\s\S]*?window-close-dialog__remember/);
  assert.match(source, /renderedMode\.kind === "normal" \? remember : false/);
  assert.match(source, /仍然退出/);
  assert.match(source, /约 1 分钟/);
  assert.match(source, /activeElement instanceof HTMLButtonElement/);
  assert.match(source, /activeElement\.dataset\.closeAction/);
  assert.match(source, /void execute\(focusedAction\)/);
  assert.doesNotMatch(source, /activeElement\.click\(\)/);
  assert.match(source, /data-default-action="true"/);
  assert.match(source, /data-close-action="tray"/);
  assert.match(source, /data-close-action="exit"/);
});

test("window close dialog follows semantic light and dark theme tokens", () => {
  const css = readProjectFile("src/app/window-lifecycle/WindowCloseDialog.css");

  assert.match(css, /\.window-close-dialog\s*\{[\s\S]*?color:\s*var\(--color-text\);/);
  assert.match(css, /background:\s*var\(--color-surface\);/);
  assert.match(css, /border:\s*1px solid var\(--color-border-muted\);/);
  assert.match(css, /\.window-close-option__copy strong\s*\{[\s\S]*?var\(--color-text\)/);
  assert.match(css, /\.window-close-dialog__success\s*\{[\s\S]*?var\(--color-surface-raised\)/);
  assert.doesNotMatch(css, /background:\s*rgba\(15, 22, 38/);
  assert.doesNotMatch(css, /background:\s*#090d16/);
  assert.doesNotMatch(css, /color:\s*#f8fafc/);
  assert.doesNotMatch(css, /color:\s*#f1f5f9/);
  assert.doesNotMatch(css, /backdrop-filter/);
});

test("window close dialog keeps closing content mounted for the CSS transition", () => {
  const source = readProjectFile("src/app/window-lifecycle/WindowCloseDialog.tsx");
  const css = readProjectFile("src/app/window-lifecycle/WindowCloseDialog.css");

  assert.match(source, /type DialogPhase = "closed" \| "opening" \| "open" \| "closing"/);
  assert.match(source, /setPhase\("closing"\)/);
  assert.match(source, /const requestCancel = useCallback/);
  assert.match(source, /setTimeout\(\(\) => \{[\s\S]*?setRenderedMode\(null\)[\s\S]*?onCancel\(\)[\s\S]*?getDialogTransitionMillis\(\)/);
  assert.match(source, /closeTimerRef/);
  assert.match(source, /requestAnimationFrame\(\(\) => \{[\s\S]*?requestAnimationFrame\(\(\) => setPhase\("open"\)\)/);
  assert.match(source, /cancelAnimationFrame\(openingFrame\)/);
  assert.match(source, /--window-close-transition-duration/);
  assert.match(css, /\.window-close-overlay\.is-opening/);
  assert.match(css, /\.window-close-overlay\.is-closing/);
  assert.match(css, /var\(--window-close-transition-duration, 200ms\)/);
  assert.match(css, /\.window-close-option\.is-default:focus/);
  assert.match(css, /prefers-reduced-motion: reduce/);
});

test("effective color scheme is synchronized to the native Tauri title bar", () => {
  const source = readProjectFile("src/app/appearance/ColorSchemeProvider.tsx");

  assert.match(source, /isTauri\(\)/);
  assert.match(source, /getCurrentWindow\(\)\.setTheme\(effective\)/);
  assert.match(readProjectFile("src-tauri/capabilities/default.json"), /core:window:allow-set-theme/);
});

test("settings window preference write reports storage failures before changing UI state", () => {
  const source = readProjectFile("src/features/settings/SettingsPage.tsx");

  assert.match(source, /windowClosePreferenceError/);
  assert.match(source, /const saveSucceeded = saveWindowClosePreference\(undefined, value\);/);
  assert.match(source, /if \(!saveSucceeded\) \{[\s\S]*?setWindowClosePreferenceError/);
  assert.match(source, /setWindowClosePreference\(value\);/);
  assert.match(source, /role="alert"/);
});
