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
  assert.match(source, /callbacksRef\.current\.onShowDialog\(\)/);
  assert.match(source, /callbacksRef\.current\.onError\(getWindowLifecycleErrorMessage\(error\)\)/);
  assert.match(source, /useEffect\(\(\) => \{[\s\S]*?\}, \[\]\);/);
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

test("settings window preference write reports storage failures before changing UI state", () => {
  const source = readProjectFile("src/features/settings/SettingsPage.tsx");

  assert.match(source, /windowClosePreferenceError/);
  assert.match(source, /const saveSucceeded = saveWindowClosePreference\(undefined, value\);/);
  assert.match(source, /if \(!saveSucceeded\) \{[\s\S]*?setWindowClosePreferenceError/);
  assert.match(source, /setWindowClosePreference\(value\);/);
  assert.match(source, /role="alert"/);
});
