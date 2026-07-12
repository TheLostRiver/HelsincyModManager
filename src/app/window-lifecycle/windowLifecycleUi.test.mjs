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

test("window close dialog traps keyboard focus and clears deferred focus", () => {
  const source = readProjectFile("src/app/window-lifecycle/WindowCloseDialog.tsx");

  assert.match(source, /FOCUSABLE_SELECTOR/);
  assert.match(source, /getFocusableDialogElements/);
  assert.match(source, /event\.key !== "Tab"/);
  assert.match(source, /event\.shiftKey/);
  assert.match(source, /firstFocusable/);
  assert.match(source, /lastFocusable/);
  assert.match(source, /clearTimeout\(focusTimer\)/);
});

test("unsafe dialog defaults focus to tray and cannot persist an exit preference", () => {
  const source = readProjectFile("src/app/window-lifecycle/WindowCloseDialog.tsx");

  assert.match(source, /mode\.kind === "unsafe"/);
  assert.match(source, /trayButtonRef/);
  assert.match(source, /mode\.kind === "unsafe"[\s\S]*?trayButtonRef\.current\?\.focus\(\)/);
  assert.match(source, /mode\.kind === "normal"[\s\S]*?window-close-dialog__remember/);
  assert.match(source, /mode\.kind === "normal" \? remember : false/);
  assert.match(source, /仍然退出/);
  assert.match(source, /约 1 分钟/);
});

test("settings window preference write reports storage failures before changing UI state", () => {
  const source = readProjectFile("src/features/settings/SettingsPage.tsx");

  assert.match(source, /windowClosePreferenceError/);
  assert.match(source, /const saveSucceeded = saveWindowClosePreference\(undefined, value\);/);
  assert.match(source, /if \(!saveSucceeded\) \{[\s\S]*?setWindowClosePreferenceError/);
  assert.match(source, /setWindowClosePreference\(value\);/);
  assert.match(source, /role="alert"/);
});
