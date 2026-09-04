import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

import { modImportCopy } from "../mods/modImportCopy.ts";
import { modImportSettingsCopy } from "./modImportSettingsCopy.ts";
import {
  getModImportSettingsErrorMessage,
  isModImportSettingsDto,
} from "./modImportSettingsTypes.ts";

function readSource(path) {
  assert.equal(existsSync(path), true, `missing source: ${path}`);
  return readFileSync(path, "utf8");
}

const LOCALES = ["zh_cn", "en", "ja"];

test("mod import settings api invokes the registered commands with the flag only", () => {
  const api = readSource("src/features/settings/modImportSettingsApi.ts");

  assert.match(api, /invoke<ModImportSettingsDto>\("get_mod_import_settings"\)/);
  assert.match(api, /invoke<ModImportSettingsDto>\("set_mod_import_settings", \{ deleteArchiveAfterImport \}\)/);
  for (const forbidden of ["archivePath", "remove", "readTextFile", "writeTextFile", "convertFileSrc"]) {
    assert.equal(api.includes(forbidden), false, `the api must never carry paths or delete itself (${forbidden})`);
  }
});

test("enabling goes through an alertdialog; disabling saves immediately", () => {
  const panel = readSource("src/features/settings/ModImportSettingsPanel.tsx");

  assert.match(panel, /if \(event\.currentTarget\.checked\) \{\s*setConfirming\(true\);\s*\} else \{\s*save\(false\);/);
  const dialogStart = panel.indexOf("function EnableConfirmDialog(");
  assert.ok(dialogStart >= 0);
  const dialog = panel.slice(dialogStart);
  assert.match(dialog, /role="alertdialog"/);
  assert.match(dialog, /closeOnBackdrop=\{false\}/);
  assert.match(dialog, /initialFocusRef=\{cancelButtonRef\}/);
  assert.match(dialog, /ref=\{cancelButtonRef\}[\s\S]*?onClick=\{onCancel\}/);
  assert.match(dialog, /copy\.confirm\.pointConsumed/);
  assert.match(dialog, /copy\.confirm\.pointCrossVolume/);
  assert.match(dialog, /copy\.confirm\.pointProtected/);
  assert.match(panel, /copy\.enabledNote/, "an enabled switch keeps a standing reminder on screen");

  const page = readSource("src/features/settings/SettingsPage.tsx");
  assert.match(page, /<ModImportSettingsPanel \/>/);
});

test("copy is satisfies-locked and every archive-kept code from the backend has import copy in three locales", () => {
  assert.match(readSource("src/features/settings/modImportSettingsCopy.ts"), /satisfies LocaleDictionary<ModImportSettingsCopy>/);
  for (const locale of LOCALES) {
    assert.equal(typeof modImportSettingsCopy[locale].confirm.confirm, "string");
  }

  const ports = readSource("src-tauri/crates/hmm-ports/src/mod_import_archive.rs");
  const codes = new Set([...ports.matchAll(/"(mod_import_archive_kept_[a-z_]+)"/g)].map((m) => m[1]));
  assert.ok(codes.size >= 5, `expected the archive-kept family, saw ${codes.size}`);
  const taskState = readSource("src/features/mods/modImportTaskState.ts");
  for (const code of codes) {
    for (const locale of LOCALES) {
      assert.equal(typeof modImportCopy[locale].archiveKept[code], "string", `${code} missing in ${locale}`);
    }
    assert.match(taskState, new RegExp(`"${code}"`), `${code} must be a recognised ModImportArchiveKeptCode`);
  }
  for (const locale of LOCALES) {
    assert.equal(typeof modImportCopy[locale].toasts.archiveKeptTitle, "string");
  }
});

test("a kept archive surfaces as a warning toast next to the success toast", () => {
  const action = readSource("src/features/mods/ModImportAction.tsx");

  assert.match(action, /if \(state\.archiveKept !== null\) \{[\s\S]*?tone: "warning"/);
  assert.match(action, /getModImportArchiveKeptMessage\(state\.archiveKept, copy\)/);
  assert.match(action, /eventKey: `mod-import\.archive-kept\.\$\{state\.taskId\}`/);
});

test("dto guard and error messages", () => {
  assert.equal(isModImportSettingsDto({ deleteArchiveAfterImport: true }), true);
  assert.equal(isModImportSettingsDto({ deleteArchiveAfterImport: "yes" }), false);
  assert.equal(isModImportSettingsDto(null), false);
  assert.equal(getModImportSettingsErrorMessage("save", "en"), modImportSettingsCopy.en.errors.saveFailed);
  assert.equal(getModImportSettingsErrorMessage("load", "ja"), modImportSettingsCopy.ja.errors.unavailableRetry);
});
