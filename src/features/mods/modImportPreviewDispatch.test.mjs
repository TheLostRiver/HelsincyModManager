// Structure only: Rust behavior tests in mod_import_commands.rs exercise the worker and input admission.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const rust = readFileSync("src-tauri/src/mod_import_commands.rs", "utf8");
test("archive preview is dispatched off the webview callback", () => {
  assert.match(rust, /pub async fn preview_dropped_mod_archives/);
  assert.match(rust, /dispatch_archive_preview\(archive_paths, probe_dropped_archives\)\.await/);
  assert.match(rust, /tauri::async_runtime::spawn_blocking\(move \|\| probe\(paths\)\)\s*\.await/);
});

test("archive preview frontend and backend share the bounded request contract", () => {
  const frontend = readFileSync("src/features/mods/modImportDropState.ts", "utf8");
  assert.match(frontend, /MAX_DROPPED_ARCHIVES = 100/);
  assert.match(rust, /MAX_DROPPED_ARCHIVES: usize = 100/);
  const limit = rust.indexOf("if archive_paths.len() > MAX_DROPPED_ARCHIVES");
  const worker = rust.indexOf("tauri::async_runtime::spawn_blocking(move || probe(paths))");
  assert.ok(limit >= 0 && worker > limit);
});
