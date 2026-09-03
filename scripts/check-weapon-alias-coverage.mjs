// Checks that every official MHW:I weapon name in the locally fetched reference tables is
// present in the bundled weapon retarget catalog (display names + aliases, per locale), so
// that "the catalog is missing aliases" is answered by running a command instead of by feel.
// #274 (2026-09-03): zero gaps in all three locales; the perceived gap was a display issue.
//
// Inputs (read-only; nothing leaves the machine):
//   <armor-data>/weapon-names.json   <opaque id> -> { zh_cn, en, ja }  three-language name table
//   <armor-data>/weapon.json         <family>    -> { <zh name>: <resource path> }
//   <artifact-dir>/mhw-weapon-targets.*.v1.json  the bundled catalog shards
//
// `armor-data/` is local-only (listed in .git/info/exclude, never committed); rebuild it with
// armor-data/scripts/fetch-weapon-names.mjs before running. This script is therefore NOT part of
// verify.ps1 — it is a maintainer tool, and its pure helpers are covered by
// check-weapon-alias-coverage.test.mjs with inline fixtures.
//
// Usage:
//   node scripts/check-weapon-alias-coverage.mjs [--json] [--samples <n>]
//   HMM_ARMOR_DATA_DIR=<dir> HMM_WEAPON_ARTIFACT_DIR=<dir> node scripts/check-weapon-alias-coverage.mjs
//
// Exit code 0 = reference and artifact name sets are identical for every locale,
//           1 = at least one gap (missing or extra name) was found,
//           2 = an input could not be read.
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

export const LOCALES = ["zh_cn", "en", "ja"];
export const DUMMY_NAME = "HARDUMMY";
const ARTIFACT_SHARD_PATTERN = /^mhw-weapon-targets\..+\.v1\.json$/;
const DEFAULT_SAMPLES = 10;

const repoRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");

// Must stay identical to `norm` in scripts/generate-weapon-catalog.mjs: the catalog was built
// with that key, so a different key here would report false gaps. The test pins both copies.
export const normalizeName = (value) =>
  (value ?? "").normalize("NFKC").replace(/[【】[\]（）()·・‧\s]/g, "").toLowerCase();

function emptyLocaleMaps() {
  return Object.fromEntries(LOCALES.map((locale) => [locale, new Map()]));
}

function remember(map, raw) {
  if (typeof raw !== "string") {
    return;
  }
  const key = normalizeName(raw);
  if (key && !map.has(key)) {
    map.set(key, raw);
  }
}

/** Reference names per locale: normalized key -> first raw spelling seen. */
export function collectReferenceNames(namesTable) {
  const maps = emptyLocaleMaps();
  for (const entry of Object.values(namesTable ?? {})) {
    for (const locale of LOCALES) {
      remember(maps[locale], entry?.[locale]);
    }
  }
  return maps;
}

/** Artifact names per locale (display name + aliases) for active targets only. */
export function collectArtifactNames(targets) {
  const maps = emptyLocaleMaps();
  for (const target of targets ?? []) {
    if (target?.status !== "active") {
      continue;
    }
    for (const locale of LOCALES) {
      const names = target.names?.[locale];
      remember(maps[locale], names?.display_name);
      for (const alias of names?.aliases ?? []) {
        remember(maps[locale], alias);
      }
    }
  }
  return maps;
}

/** zh names of the name -> model path table, minus the game's placeholder entry. */
export function collectPathTableNames(weaponJson, dummyName = DUMMY_NAME) {
  const map = new Map();
  for (const entries of Object.values(weaponJson ?? {})) {
    for (const name of Object.keys(entries ?? {})) {
      if (name !== dummyName) {
        remember(map, name);
      }
    }
  }
  return map;
}

function difference(left, right) {
  return [...left.entries()].filter(([key]) => !right.has(key)).map(([, raw]) => raw);
}

export function compareCoverage({ reference, artifact, pathTable = new Map() }) {
  const locales = {};
  let ok = true;
  for (const locale of LOCALES) {
    const missing = difference(reference[locale], artifact[locale]);
    const extra = difference(artifact[locale], reference[locale]);
    locales[locale] = {
      referenceCount: reference[locale].size,
      artifactCount: artifact[locale].size,
      missing,
      extra,
    };
    ok = ok && missing.length === 0 && extra.length === 0;
  }
  const pathTableReport = {
    count: pathTable.size,
    missingFromReference: difference(pathTable, reference.zh_cn),
    missingFromArtifact: difference(pathTable, artifact.zh_cn),
  };
  ok =
    ok &&
    pathTableReport.missingFromReference.length === 0 &&
    pathTableReport.missingFromArtifact.length === 0;
  return { ok, locales, pathTable: pathTableReport };
}

function sampleList(label, values, samples) {
  if (values.length === 0) {
    return [];
  }
  const shown = values.slice(0, samples).join(" | ");
  const rest = values.length > samples ? `… +${values.length - samples}` : "";
  return [`      ${label}: ${[shown, rest].filter(Boolean).join(" ")}`];
}

export function renderReport(result, { samples = DEFAULT_SAMPLES } = {}) {
  const lines = ["=== weapon alias coverage (reference vs bundled catalog) ==="];
  for (const locale of LOCALES) {
    const report = result.locales[locale];
    const status = report.missing.length === 0 && report.extra.length === 0 ? "PASS" : "FAIL";
    lines.push(
      `${status}  ${locale}: reference ${report.referenceCount} names, artifact ${report.artifactCount} names, missing ${report.missing.length}, extra ${report.extra.length}`,
    );
    lines.push(...sampleList("missing (reference has, artifact lacks)", report.missing, samples));
    lines.push(...sampleList("extra (artifact has, reference lacks)", report.extra, samples));
  }
  const table = result.pathTable;
  const tableStatus =
    table.missingFromReference.length === 0 && table.missingFromArtifact.length === 0
      ? "PASS"
      : "FAIL";
  lines.push(
    `${tableStatus}  weapon.json (zh name -> model path): ${table.count} names, not in reference ${table.missingFromReference.length}, not in artifact ${table.missingFromArtifact.length}`,
  );
  lines.push(...sampleList("not in reference", table.missingFromReference, samples));
  lines.push(...sampleList("not in artifact", table.missingFromArtifact, samples));
  lines.push("");
  lines.push(result.ok ? "coverage complete: no gaps in any locale" : "coverage gaps found");
  return lines;
}

function parseArgs(argv) {
  const options = { json: false, samples: DEFAULT_SAMPLES };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--json") {
      options.json = true;
    } else if (arg === "--samples") {
      const value = Number.parseInt(argv[index + 1] ?? "", 10);
      if (!Number.isInteger(value) || value < 0) {
        throw new Error("--samples expects a non-negative integer");
      }
      options.samples = value;
      index += 1;
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  return options;
}

function readJson(filePath) {
  return JSON.parse(readFileSync(filePath, "utf8"));
}

export function loadArtifactTargets(artifactDir) {
  const shards = readdirSync(artifactDir)
    .filter((name) => ARTIFACT_SHARD_PATTERN.test(name))
    .sort();
  if (shards.length === 0) {
    throw new Error(`no mhw-weapon-targets.*.v1.json shard found in ${artifactDir}`);
  }
  return shards.flatMap((shard) => readJson(path.join(artifactDir, shard)).targets ?? []);
}

export function main(argv = process.argv.slice(2), env = process.env) {
  let options;
  try {
    options = parseArgs(argv);
  } catch (error) {
    console.error(error.message);
    return 2;
  }

  const armorDataDir = env.HMM_ARMOR_DATA_DIR ?? path.join(repoRoot, "armor-data");
  const artifactDir =
    env.HMM_WEAPON_ARTIFACT_DIR ??
    path.join(repoRoot, "src-tauri", "crates", "hmm-games-mhw", "data", "weapons");
  const namesPath = path.join(armorDataDir, "weapon-names.json");
  const weaponPath = path.join(armorDataDir, "weapon.json");

  for (const required of [namesPath, weaponPath]) {
    if (!existsSync(required)) {
      console.error(`reference data not found: ${required}`);
      console.error(
        "armor-data/ is local-only (not in git). Fetch it with armor-data/scripts/fetch-weapon-names.mjs, or point HMM_ARMOR_DATA_DIR at a directory that has weapon-names.json and weapon.json.",
      );
      return 2;
    }
  }

  let result;
  try {
    result = compareCoverage({
      reference: collectReferenceNames(readJson(namesPath)),
      artifact: collectArtifactNames(loadArtifactTargets(artifactDir)),
      pathTable: collectPathTableNames(readJson(weaponPath)),
    });
  } catch (error) {
    console.error(`cannot read inputs: ${error.message}`);
    return 2;
  }

  if (options.json) {
    console.log(JSON.stringify(result, null, 2));
  } else {
    for (const line of renderReport(result, { samples: options.samples })) {
      console.log(line);
    }
  }
  return result.ok ? 0 : 1;
}

// Guarded so importing this module for its pure helpers does not read the local data and
// exit the host process.
const isDirectRun =
  process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href;

if (isDirectRun) {
  process.exit(main());
}
