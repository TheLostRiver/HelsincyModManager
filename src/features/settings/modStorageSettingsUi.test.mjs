import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  assert.equal(existsSync(path), true, `missing source: ${path}`);
  return readFileSync(path, "utf8");
}

test("mod storage api wrappers invoke the registered commands with narrow payloads", () => {
  const api = readSource("src/features/settings/modStorageApi.ts");

  assert.match(api, /invoke<ModStorageSettingsDto>\("get_mod_storage_settings"\)/);
  assert.match(api, /invoke<ModStorageDirValidationDto>\("validate_mod_storage_dir", \{ directory \}\)/);
  assert.match(api, /invoke<ModStorageSettingsDto>\("set_mod_storage_dir", \{ directory \}\)/);
  assert.match(api, /invoke<TaskStartedDto>\("start_mod_storage_migration_task", \{ directory \}\)/);
  assert.match(api, /invoke<TaskStartedDto>\("cancel_task", \{ taskId \}\)/);
  for (const forbidden of ["readTextFile", "writeTextFile", "readDir", "remove", "convertFileSrc", "sandboxes"]) {
    assert.equal(api.includes(forbidden), false, `api must not touch the file system itself (${forbidden})`);
  }
});

test("the change confirmation is an alertdialog that cannot be dismissed by the backdrop", () => {
  const panel = readSource("src/features/settings/ModStorageSettingsPanel.tsx");
  const dialogStart = panel.indexOf("function ModStorageChangeDialog(");
  assert.ok(dialogStart >= 0);
  const dialog = panel.slice(dialogStart);

  assert.match(dialog, /role="alertdialog"/);
  assert.match(dialog, /closeOnBackdrop=\{false\}/);
  assert.match(dialog, /initialFocusRef=\{cancelButtonRef\}/);
  assert.match(dialog, /ref=\{cancelButtonRef\}[\s\S]*?onClick=\{onCancel\}/, "focus lands on Cancel");
  assert.match(dialog, /copy\.confirm\.migrateStepCopy/);
  assert.match(dialog, /copy\.confirm\.migrateStepFreeze/);
  assert.match(dialog, /copy\.confirm\.migrateStepRestart/);
});

test("the panel projects backend facts and never recomputes the gate", () => {
  const panel = readSource("src/features/settings/ModStorageSettingsPanel.tsx");
  const hook = readSource("src/features/settings/useModStorageSettings.ts");

  assert.match(panel, /settings\.writesFrozen !== "none"/);
  assert.match(panel, /getModStorageDegradedMessage\(settings\.degradedReason, locale\)/);
  assert.match(panel, /role="alert"/);
  assert.match(panel, /role="progressbar"/);
  assert.match(panel, /canCancelModStorageMigration\(migration\)/);
  assert.match(hook, /settings\?\.writesFrozen \?\? "none"/);
  assert.match(hook, /open\(\{ directory: true, multiple: false, title: copy\.actions\.pickerTitle \}\)/);
  assert.match(hook, /validateModStorageDir\(directory\)/);
  assert.match(hook, /settings\.libraryEmpty \? "set" : "migrate"/, "set vs migrate follows the backend libraryEmpty fact");
  assert.match(hook, /event\.payload\.kind !== "mod_storage_migration"/);
  assert.match(hook, /isTaskStartedDto\(started, "mod_storage_migration"\)/);
  assert.doesNotMatch(hook, /restartRequired &&/, "the freeze must not be derived from restartRequired");
  for (const source of [panel, hook]) {
    for (const forbidden of ["readTextFile", "writeTextFile", "convertFileSrc", "sandboxes/"]) {
      assert.equal(source.includes(forbidden), false, `must not touch files directly (${forbidden})`);
    }
  }
});

test("the provider sits above the router and the settings page mounts the section", () => {
  const app = readSource("src/App.tsx");
  const providerIndex = app.indexOf("<ModStorageSettingsProvider>");
  const outletIndex = app.indexOf("<RouterOutlet />");
  const feedbackIndex = app.indexOf("<FeedbackProvider>");
  assert.ok(providerIndex >= 0 && outletIndex > providerIndex, "provider must wrap the router outlet");
  assert.ok(feedbackIndex >= 0 && feedbackIndex < providerIndex, "the hook pushes toasts, so FeedbackProvider is outside");

  const page = readSource("src/features/settings/SettingsPage.tsx");
  assert.match(page, /<ModStorageSettingsPanel \/>/);
  assert.match(page, /title=\{storageCopy\.section\.title\}/);
  assert.match(page, /tourId="settings\.mod-storage"/);
});

test("import and delete entry points take the freeze reason from the shared snapshot", () => {
  const library = readSource("src/features/mods/ModLibraryPage.tsx");
  const compact = readSource("src/features/mods/CompactActionPanel.tsx");
  const external = readSource("src/features/mods/external-import/ExternalImportAction.tsx");

  assert.match(library, /const modStorage = useModStorageSettings\(\);/);
  assert.match(library, /getModStorageFreezeReason\(modStorage\.writesFrozen, locale\)/);
  assert.match(library, /storageWriteFreezeReason=\{storageWriteFreezeReason\}/);
  assert.match(library, /if \(storageWriteFreezeReason !== undefined\) \{\s*return \{ label, disabledReason: storageWriteFreezeReason \} as const;/);

  assert.match(compact, /storageWriteFreezeReason\?: string;/);
  assert.match(compact, /<ModImportAction\s+label=\{buttonText\.add\}\s+disabledReason=\{storageWriteFreezeReason\}/);
  assert.match(compact, /<ExternalImportAction onImported=\{onImportCompleted\} disabledReason=\{storageWriteFreezeReason\} \/>/);
  assert.match(compact, /storageWriteFreezeReason !== undefined\s*\?\s*storageWriteFreezeReason/, "revision import cascade starts with the freeze");
  assert.match(compact, /if \(actionId === "delete"\) \{\s*return storageWriteFreezeReason;/, "batch delete respects the freeze");

  assert.match(external, /disabledReason\?: string;/);
  assert.match(external, /disabled=\{listenerStatus === "loading" \|\| disabledReason !== undefined\}/);
});

test("gate codes returned by import / delete / external import commands have specific copy", () => {
  const importCopy = readSource("src/features/mods/modImportCopy.ts");
  const importAction = readSource("src/features/mods/ModImportAction.tsx");
  const deleteCopy = readSource("src/features/mods/modDeleteCopy.ts");
  const externalCopy = readSource("src/features/mods/external-import/externalImportCopy.ts");

  assert.match(importAction, /code === "mod_storage_migration_in_progress"[\s\S]*?"storage-frozen-migration"/);
  assert.match(importAction, /code === "mod_storage_restart_required"[\s\S]*?"storage-frozen-restart"/);
  for (const key of ["storageFrozenMigration", "storageFrozenRestart"]) {
    assert.equal((importCopy.match(new RegExp(`${key}:`, "g")) ?? []).length, 4, `${key}: type + 3 locales`);
  }
  for (const code of ["mod_storage_migration_in_progress", "mod_storage_restart_required"]) {
    assert.equal((deleteCopy.match(new RegExp(code, "g")) ?? []).length, 3, `${code} in modDeleteCopy: 3 locales`);
    // selection.errors + result.errors per locale
    assert.equal((externalCopy.match(new RegExp(code, "g")) ?? []).length, 6, `${code} in externalImportCopy: 2 maps × 3 locales`);
  }
});
