import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const scriptsDir = dirname(fileURLToPath(import.meta.url));
const script = join(scriptsDir, "check-storage-reclaim.mjs");
const cleanups = [];

function buildAppData({ mods, revisions, sandboxes = [], thumbnails = null, manifest = null }) {
  const root = mkdtempSync(join(tmpdir(), "hmm-reclaim-"));
  cleanups.push(root);
  mkdirSync(join(root, "mod-import/sandboxes"), { recursive: true });
  mkdirSync(join(root, "install/manifests"), { recursive: true });
  writeFileSync(
    join(root, "mod-import/results.json"),
    JSON.stringify({ version: 1, mods, revisions }),
  );
  for (const entry of sandboxes) {
    mkdirSync(join(root, "mod-import/sandboxes", entry), { recursive: true });
  }
  if (thumbnails !== null) {
    mkdirSync(join(root, "thumbnails"), { recursive: true });
    for (const entry of thumbnails) {
      mkdirSync(join(root, "thumbnails", entry), { recursive: true });
    }
  }
  if (manifest !== null) {
    writeFileSync(
      join(root, "install/manifests/default.json"),
      JSON.stringify(manifest),
    );
  }
  return root;
}

function run(root, extraArgs = []) {
  return spawnSync(process.execPath, [script, ...extraArgs], {
    env: { ...process.env, HMM_APP_DATA_DIR: root },
    encoding: "utf8",
  });
}

test.after(() => {
  for (const root of cleanups) {
    rmSync(root, { recursive: true, force: true });
  }
});

test("passes when catalog, sandboxes and thumbnails agree", () => {
  const root = buildAppData({
    mods: [{ mod_id: "pkg-1" }],
    revisions: [{ package_id: "pkg-1" }],
    sandboxes: ["pkg-1"],
    thumbnails: ["pkg-1"],
    manifest: { entries: [] },
  });

  const result = run(root);
  assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
  assert.match(result.stdout, /all 4 checks passed/);
});

test("passes when the thumbnail cache dir does not exist yet", () => {
  const root = buildAppData({
    mods: [{ mod_id: "pkg-1" }],
    revisions: [{ package_id: "pkg-1" }],
    sandboxes: ["pkg-1"],
    thumbnails: null,
  });

  const result = run(root);
  assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
  assert.match(result.stdout, /thumbnails dir absent/);
});

test("fails when a sandbox directory is orphaned", () => {
  const root = buildAppData({
    mods: [{ mod_id: "pkg-1" }],
    revisions: [{ package_id: "pkg-1" }],
    sandboxes: ["pkg-1", "pkg-ghost"],
  });

  const result = run(root);
  assert.equal(result.status, 1);
  assert.match(result.stdout, /FAIL {2}no orphan sandbox directory/);
  assert.match(result.stdout, /pkg-ghost/);
});

test("fails when a thumbnail directory is orphaned", () => {
  const root = buildAppData({
    mods: [{ mod_id: "pkg-1" }],
    revisions: [{ package_id: "pkg-1" }],
    sandboxes: ["pkg-1"],
    thumbnails: ["pkg-deleted"],
  });

  const result = run(root);
  assert.equal(result.status, 1);
  assert.match(result.stdout, /FAIL {2}no orphan thumbnail directory/);
  assert.match(result.stdout, /pkg-deleted/);
});

test("fails when the install manifest still references a deleted package", () => {
  const root = buildAppData({
    mods: [{ mod_id: "pkg-1" }],
    revisions: [{ package_id: "pkg-1" }],
    sandboxes: ["pkg-1"],
    // A real dangling reference is always a real id - the package existed once.
    manifest: {
      entries: [
        {
          mod_id: "mod-import-1787999000000-0",
          revision_id: "mod-import-1787999000000-0",
        },
      ],
    },
  });

  const result = run(root);
  assert.equal(result.status, 1);
  assert.match(result.stdout, /manifest references no deleted package/);
});

test("reference extraction does not depend on the id format", () => {
  // Guards against regressing back to regex-scraping the manifest. A pattern tied
  // to today's id shapes keeps matching today's ids, so the case above stays
  // green while the checker silently stops seeing anything else. An unfamiliar
  // format must still be reported.
  const root = buildAppData({
    mods: [{ mod_id: "pkg-1" }],
    revisions: [{ package_id: "pkg-1" }],
    sandboxes: ["pkg-1"],
    manifest: {
      entries: [
        { mod_id: "some-future-id-format", revision_id: "some-future-id-format" },
      ],
    },
  });

  const result = run(root);
  assert.equal(result.status, 1);
  assert.match(result.stdout, /some-future-id-format/);
});

test("confirms a deleted package is gone from every location", () => {
  const root = buildAppData({
    mods: [{ mod_id: "pkg-1" }],
    revisions: [{ package_id: "pkg-1" }],
    sandboxes: ["pkg-1"],
    thumbnails: [],
  });

  const result = run(root, ["--deleted", "mod-import-1788072641961-0"]);
  assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
  assert.match(result.stdout, /PASS {2}deleted package fully reclaimed/);
  assert.match(result.stdout, /all 5 checks passed/);
});

test("fails when the deleted package is still present", () => {
  const root = buildAppData({
    mods: [{ mod_id: "pkg-1" }, { mod_id: "pkg-2" }],
    revisions: [{ package_id: "pkg-1" }, { package_id: "pkg-2" }],
    sandboxes: ["pkg-1", "pkg-2"],
    thumbnails: ["pkg-2"],
  });

  const result = run(root, ["--deleted", "pkg-2"]);
  assert.equal(result.status, 1);
  assert.match(result.stdout, /still in: catalog, mods, sandboxes, thumbnails/);
});

test("reads sandboxes from the configured mod storage directory (#275)", () => {
  const root = buildAppData({
    mods: [{ mod_id: "pkg-1" }],
    revisions: [{ package_id: "pkg-1" }],
    sandboxes: [],
    thumbnails: ["pkg-1"],
    manifest: { entries: [] },
  });
  const storageRoot = mkdtempSync(join(tmpdir(), "hmm-storage-"));
  cleanups.push(storageRoot);
  mkdirSync(join(storageRoot, "sandboxes", "pkg-1"), { recursive: true });
  mkdirSync(join(root, "config"), { recursive: true });
  writeFileSync(
    join(root, "config/settings.json"),
    JSON.stringify({ version: 1, modStorageDir: storageRoot }),
  );

  const result = run(root);
  assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
  assert.match(result.stdout, /all 4 checks passed/);
  assert.ok(
    result.stdout.includes(join(storageRoot, "sandboxes")),
    `expected the configured sandboxes dir to be reported: ${result.stdout}`,
  );
});

test("ignores a relative modStorageDir and falls back to app data", () => {
  const root = buildAppData({
    mods: [{ mod_id: "pkg-1" }],
    revisions: [{ package_id: "pkg-1" }],
    sandboxes: ["pkg-1"],
    thumbnails: null,
    manifest: null,
  });
  mkdirSync(join(root, "config"), { recursive: true });
  writeFileSync(
    join(root, "config/settings.json"),
    JSON.stringify({ version: 1, modStorageDir: "relative/mods" }),
  );

  const result = run(root);
  assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
  assert.ok(result.stdout.includes(join(root, "mod-import", "sandboxes")));
});

test("fails with a clear message when the app data dir is missing", () => {
  const result = spawnSync(process.execPath, [script], {
    env: { ...process.env, HMM_APP_DATA_DIR: join(tmpdir(), "hmm-does-not-exist-xyz") },
    encoding: "utf8",
  });

  assert.equal(result.status, 1);
  assert.match(result.stderr, /app data dir not found/);
  assert.match(result.stderr, /HMM_APP_DATA_DIR/);
});
