import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  buildProfile,
  hostTripleFromRustc,
  resolveTargetTriple,
  sidecarFileName,
  targetDirectoryFromCargoMetadata,
  workerBuildTauriConfig,
} from "./prepare-save-backup-worker-sidecar.mjs";

test("parses rustc host triple", () => {
  assert.equal(
    hostTripleFromRustc("rustc 1.95.0\nhost: x86_64-pc-windows-msvc\n"),
    "x86_64-pc-windows-msvc",
  );
  assert.throws(
    () => hostTripleFromRustc("rustc 1.95.0\n"),
    /host triple/,
  );
});

test("uses Tauri target-triple sidecar naming", () => {
  assert.equal(
    sidecarFileName("x86_64-pc-windows-msvc"),
    "hmm-save-backup-worker-x86_64-pc-windows-msvc.exe",
  );
  assert.equal(
    sidecarFileName("x86_64-unknown-linux-gnu"),
    "hmm-save-backup-worker-x86_64-unknown-linux-gnu",
  );
});

test("uses cargo metadata target directory and explicit profiles", () => {
  assert.equal(
    targetDirectoryFromCargoMetadata('{"target_directory":"D:/cargo-target"}'),
    path.normalize("D:/cargo-target"),
  );
  assert.equal(buildProfile([]), "release");
  assert.equal(buildProfile(["--debug"]), "debug");
  assert.throws(
    () => buildProfile(["--unknown"]),
    /unknown sidecar argument/,
  );
  assert.throws(
    () => targetDirectoryFromCargoMetadata("{}"),
    /target directory/,
  );
});

test("rejects unsafe or conflicting target triple input", () => {
  for (const target of ["", "../windows", "x86_64/windows", "x86_64\\windows"]) {
    assert.throws(() => sidecarFileName(target), /invalid Rust target triple/);
  }
  assert.throws(
    () =>
      resolveTargetTriple(
        "aarch64-pc-windows-msvc",
        "x86_64-pc-windows-msvc",
        "x86_64",
      ),
    /does not match TAURI_ENV_ARCH/,
  );
});

test("resolves an explicit target before the host target", () => {
  assert.equal(
    resolveTargetTriple(
      "aarch64-pc-windows-msvc",
      "x86_64-pc-windows-msvc",
      "aarch64",
    ),
    "aarch64-pc-windows-msvc",
  );
  assert.equal(
    resolveTargetTriple(undefined, "x86_64-pc-windows-msvc", undefined),
    "x86_64-pc-windows-msvc",
  );
});

test("disables external binaries only for the worker cargo build", () => {
  assert.deepEqual(JSON.parse(workerBuildTauriConfig(undefined)), {
    bundle: { externalBin: [] },
  });

  const existingConfig = JSON.stringify({
    build: { devUrl: "http://localhost:1420" },
    bundle: {
      active: true,
      externalBin: ["binaries/hmm-save-backup-worker"],
    },
  });
  assert.deepEqual(JSON.parse(workerBuildTauriConfig(existingConfig)), {
    build: { devUrl: "http://localhost:1420" },
    bundle: { active: true, externalBin: [] },
  });
});

test("rejects invalid worker cargo build Tauri configuration", () => {
  assert.throws(
    () => workerBuildTauriConfig("{invalid"),
    /TAURI_CONFIG must be valid JSON/,
  );
  assert.throws(
    () => workerBuildTauriConfig("[]"),
    /TAURI_CONFIG must be a JSON object/,
  );
  assert.throws(
    () => workerBuildTauriConfig('{"bundle":[]}'),
    /TAURI_CONFIG bundle must be a JSON object/,
  );
});

test("keeps the GUI binary as the Cargo default run target", () => {
  const repoRoot = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "..",
  );
  const result = spawnSync(
    "cargo",
    ["metadata", "--format-version", "1", "--no-deps"],
    { cwd: repoRoot, encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);

  const metadata = JSON.parse(result.stdout);
  const tauriPackage = metadata.packages.find(
    (candidate) => candidate.name === "hmm-tauri",
  );
  assert.equal(tauriPackage?.default_run, "hmm-tauri");
});
