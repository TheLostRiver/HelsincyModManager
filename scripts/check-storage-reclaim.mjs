// Verifies that deleting a Mod actually reclaims storage: the revision catalog,
// sandbox directories, thumbnail cache and install manifest must stay consistent
// with each other - no orphans, no dangling references.
//
// Usage:
//   node scripts/check-storage-reclaim.mjs
//   node scripts/check-storage-reclaim.mjs --deleted <packageId>
//   HMM_APP_DATA_DIR=<path> node scripts/check-storage-reclaim.mjs
//
// Exit code 0 = every check passed, 1 = at least one failed.
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";

export function collectPackageIds(revisions) {
  return new Set((revisions ?? []).map((revision) => revision?.package_id).filter(Boolean));
}

export function collectModIds(mods) {
  return new Set((mods ?? []).map((mod) => mod?.mod_id).filter(Boolean));
}

// Read the ids structurally rather than pattern-matching the whole document:
// the manifest stores mod_id / revision_id, not package_id, and a regex tied to
// today's id formats silently stops matching the day a new format appears.
export function collectManifestReferences(manifest) {
  if (manifest === null || manifest === undefined) {
    return [];
  }

  const ids = [];
  for (const entry of manifest.entries ?? []) {
    ids.push(entry?.mod_id, entry?.revision_id);
  }
  for (const binding of manifest.replacement_bindings ?? []) {
    ids.push(binding?.binding?.mod_id, binding?.revision_id);
  }

  return [...new Set(ids.filter(Boolean))];
}

export function evaluateStorageReclaim(input) {
  const {
    mods = [],
    revisions = [],
    sandboxes = [],
    thumbnails = null,
    manifestIds = [],
    deletedPackageId = null,
  } = input;

  const packageIds = collectPackageIds(revisions);
  const modIds = collectModIds(mods);
  const checks = [];
  const add = (name, pass, detail) => {
    checks.push({ name, pass, detail });
  };

  add(
    "sandboxes match catalog 1:1",
    sandboxes.length === revisions.length &&
      sandboxes.every((entry) => packageIds.has(entry)),
    `sandboxes=${sandboxes.length} revisions=${revisions.length}`,
  );

  const orphanSandboxes = sandboxes.filter((entry) => !packageIds.has(entry));
  add(
    "no orphan sandbox directory",
    orphanSandboxes.length === 0,
    orphanSandboxes.length ? orphanSandboxes.join(" ") : "none",
  );

  if (thumbnails === null) {
    add("no orphan thumbnail directory", true, "thumbnails dir absent");
  } else {
    const orphanThumbnails = thumbnails.filter((entry) => !packageIds.has(entry));
    add(
      "no orphan thumbnail directory",
      orphanThumbnails.length === 0,
      orphanThumbnails.length
        ? orphanThumbnails.join(" ")
        : `${thumbnails.length} dir(s), all referenced`,
    );
  }

  const dangling = manifestIds.filter((id) => !packageIds.has(id));
  add(
    "manifest references no deleted package",
    dangling.length === 0,
    dangling.length ? dangling.join(" ") : manifestIds.join(" ") || "no refs",
  );

  if (deletedPackageId) {
    const stillIn = [
      packageIds.has(deletedPackageId) ? "catalog" : null,
      modIds.has(deletedPackageId) ? "mods" : null,
      sandboxes.includes(deletedPackageId) ? "sandboxes" : null,
      (thumbnails ?? []).includes(deletedPackageId) ? "thumbnails" : null,
    ].filter(Boolean);

    add(
      "deleted package fully reclaimed",
      stillIn.length === 0,
      stillIn.length ? `still in: ${stillIn.join(", ")}` : "gone from every location",
    );
  }

  return checks;
}

function readJson(file) {
  try {
    return JSON.parse(readFileSync(file, "utf8"));
  } catch {
    return null;
  }
}

function listDir(dir) {
  return existsSync(dir) ? readdirSync(dir) : null;
}

function parseArgs(argv) {
  const flagIndex = argv.indexOf("--deleted");
  const value = flagIndex === -1 ? null : argv[flagIndex + 1];
  return { deletedPackageId: value && !value.startsWith("--") ? value : null };
}

function resolveAppDataDir() {
  if (process.env.HMM_APP_DATA_DIR) {
    return process.env.HMM_APP_DATA_DIR;
  }
  return process.env.APPDATA
    ? path.join(process.env.APPDATA, "dev.helsincy.modmanager")
    : null;
}

function main() {
  const { deletedPackageId } = parseArgs(process.argv.slice(2));
  const root = resolveAppDataDir();

  if (root === null || !existsSync(root)) {
    console.error(`app data dir not found: ${root ?? "(unset)"}`);
    console.error("set HMM_APP_DATA_DIR to point at the app data root.");
    return 1;
  }

  const results = readJson(path.join(root, "mod-import/results.json"));
  if (results === null) {
    console.error(`cannot read ${path.join(root, "mod-import/results.json")}`);
    return 1;
  }

  const mods = results.mods ?? [];
  const revisions = results.revisions ?? [];
  const checks = evaluateStorageReclaim({
    mods,
    revisions,
    sandboxes: listDir(path.join(root, "mod-import/sandboxes")) ?? [],
    thumbnails: listDir(path.join(root, "thumbnails")),
    manifestIds: collectManifestReferences(
      readJson(path.join(root, "install/manifests/default.json")),
    ),
    deletedPackageId,
  });

  console.log("=== storage reclaim check ===");
  console.log(`app data:  ${root}`);
  console.log(`mods:      ${mods.length}`);
  console.log(`revisions: ${revisions.length}`);
  console.log("");
  for (const check of checks) {
    console.log(`${check.pass ? "PASS" : "FAIL"}  ${check.name}`);
    console.log(`      ${check.detail}`);
  }

  const failed = checks.filter((check) => !check.pass).length;
  console.log("");
  console.log(
    failed === 0
      ? `all ${checks.length} checks passed`
      : `${failed} check(s) FAILED`,
  );
  return failed === 0 ? 0 : 1;
}

process.exit(main());
