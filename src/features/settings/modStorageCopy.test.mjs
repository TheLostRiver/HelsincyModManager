import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

import { modStorageCopy } from "./modStorageCopy.ts";
import {
  getModStorageErrorMessage,
  getModStorageFreezeReason,
  isModStorageDirValidationDto,
  isModStorageSettingsDto,
  modStorageErrorCodeFrom,
} from "./modStorageTypes.ts";

function readSource(path) {
  assert.equal(existsSync(path), true, `missing source: ${path}`);
  return readFileSync(path, "utf8");
}

/** Every `"mod_storage_…"` literal a Rust `code()` function can return in the given files. */
function rustCodes(paths, prefix) {
  const codes = new Set();
  for (const path of paths) {
    for (const match of readSource(path).matchAll(new RegExp(`"(${prefix}[a-z0-9_]+)"`, "g"))) {
      codes.add(match[1]);
    }
  }
  return codes;
}

const LOCALES = ["zh_cn", "en", "ja"];

test("directory codes: every backend mod_storage_dir_* code has copy in all three locales", () => {
  const codes = rustCodes(["src-tauri/crates/hmm-ports/src/mod_storage.rs"], "mod_storage_dir_");
  assert.ok(codes.size >= 12, `expected the full directory code family, saw ${codes.size}`);
  for (const code of codes) {
    for (const locale of LOCALES) {
      assert.equal(typeof modStorageCopy[locale].errors[code], "string", `${code} missing in ${locale}`);
    }
  }
});

test("migration codes: every code a task event or start command can carry has copy", () => {
  // Terminal event codes come from ModStorageMigrationError; the settlement-only
  // `target_package_missing` never reaches an event (it is logged at startup instead).
  const eventCodes = rustCodes(["src-tauri/crates/hmm-ports/src/mod_storage.rs"], "mod_storage_migration_");
  eventCodes.delete("mod_storage_migration_target_package_missing");
  const appCodes = rustCodes(
    [
      "src-tauri/crates/hmm-app/src/mod_storage_migration.rs",
      "src-tauri/crates/hmm-app/src/mod_storage_write_gate.rs",
      "src-tauri/crates/hmm-app/src/mod_storage_settings.rs",
    ],
    "mod_storage_",
  );
  for (const code of [...eventCodes, ...appCodes]) {
    if (code.startsWith("mod_storage_dir_") || code === "mod_storage_migration_target_package_missing") {
      continue;
    }
    for (const locale of LOCALES) {
      assert.equal(typeof modStorageCopy[locale].errors[code], "string", `${code} missing in ${locale}`);
    }
  }
  for (const code of ["app_settings_unavailable", "mod_library_unavailable", "game_config_unavailable"]) {
    for (const locale of LOCALES) {
      assert.equal(typeof modStorageCopy[locale].errors[code], "string", `${code} missing in ${locale}`);
    }
  }
});

test("every phase in the contract table has a label in all three locales", () => {
  const contract = readSource("docs/FRONTEND_BACKEND_CONTRACT.md");
  const phases = new Set(
    [...contract.matchAll(/`mod_storage_migration` \| `(mod_storage\.migration\.[a-z_]+)`/g)].map((m) => m[1]),
  );
  assert.equal(phases.size, 8, "contract registers 8 migration phases");
  for (const phase of phases) {
    for (const locale of LOCALES) {
      assert.equal(typeof modStorageCopy[locale].migration.phases[phase], "string", `${phase} missing in ${locale}`);
    }
  }
});

test("copy dictionary is satisfies-locked and the unknown fallback keeps the code visible", () => {
  const source = readSource("src/features/settings/modStorageCopy.ts");
  assert.match(source, /satisfies LocaleDictionary<ModStorageCopy>/);
  assert.equal(getModStorageErrorMessage("mod_storage_dir_not_writable", "en"), modStorageCopy.en.errors.mod_storage_dir_not_writable);
  assert.equal(getModStorageErrorMessage("mod_storage_dir_not_writable", "zh_cn"), modStorageCopy.zh_cn.errors.mod_storage_dir_not_writable);
  assert.match(getModStorageErrorMessage("something_new", "en"), /something_new/);
  assert.match(getModStorageErrorMessage("unknown", "en"), /unknown/, "the literal `unknown` key must not resolve to the fallback function");
});

test("freeze reasons project the backend writesFrozen fact only", () => {
  assert.equal(getModStorageFreezeReason("none", "en"), undefined);
  assert.equal(getModStorageFreezeReason("migration", "en"), modStorageCopy.en.frozen.migration);
  assert.equal(getModStorageFreezeReason("restart_required", "ja"), modStorageCopy.ja.frozen.restart_required);
});

test("DTO guards accept the contract shape and reject drift", () => {
  const dto = {
    effectiveDir: "E:/HMMMods",
    defaultDir: "C:/app-data/mod-import",
    configuredDir: "E:/HMMMods",
    source: "configured",
    libraryEmpty: false,
    restartRequired: false,
    writesFrozen: "none",
  };
  assert.equal(isModStorageSettingsDto(dto), true);
  assert.equal(isModStorageSettingsDto({ ...dto, degradedReason: "configured_dir_unavailable", degradedDetail: "mod_storage_dir_marker_required" }), true);
  assert.equal(isModStorageSettingsDto({ ...dto, writesFrozen: "frozen" }), false);
  assert.equal(isModStorageSettingsDto({ ...dto, source: "elsewhere" }), false);
  assert.equal(isModStorageSettingsDto({ ...dto, configuredDir: undefined }), false);
  assert.equal(isModStorageSettingsDto(null), false);
  assert.equal(isModStorageDirValidationDto({ ok: false, code: "mod_storage_dir_unsafe", exists: false, claimed: false }), true);
  assert.equal(isModStorageDirValidationDto({ ok: true, exists: true, claimed: true }), false);
  assert.equal(modStorageErrorCodeFrom({ code: "mod_storage_dir_unsafe" }, "app_settings_unavailable"), "mod_storage_dir_unsafe");
  assert.equal(modStorageErrorCodeFrom(new Error("boom"), "app_settings_unavailable"), "app_settings_unavailable");
  assert.equal(modStorageErrorCodeFrom({ code: "  " }, "app_settings_unavailable"), "app_settings_unavailable");
});
