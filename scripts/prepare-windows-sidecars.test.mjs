import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  WINDOWS_SIDECAR_BINARIES,
  assertSidecarBuildOutput,
  buildProfile,
  capturedCommandFailure,
  hostTripleFromRustc,
  resolveTargetTriple,
  sidecarFileName,
  sidecarFileNames,
  targetDirectoryFromCargoMetadata,
  windowsSidecarBuildTauriConfig,
} from "./prepare-windows-sidecars.mjs";

const worker = "hmm-save-backup-worker";
const installerCleanup = "hmm-save-backup-installer-cleanup";

test("uses a fixed Windows sidecar allowlist", () => {
  assert.deepEqual(WINDOWS_SIDECAR_BINARIES, [worker, installerCleanup]);
  assert.throws(
    () => sidecarFileName("arbitrary-helper", "x86_64-pc-windows-msvc"),
    /unsupported Windows sidecar binary/,
  );
  assert.throws(
    () =>
      sidecarFileNames(
        [worker, worker],
        "x86_64-pc-windows-msvc",
      ),
    /duplicate Windows sidecar binary/,
  );
});

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

test("uses Tauri target-triple names for both sidecars", () => {
  assert.deepEqual(
    sidecarFileNames(
      WINDOWS_SIDECAR_BINARIES,
      "x86_64-pc-windows-msvc",
    ),
    [
      `${worker}-x86_64-pc-windows-msvc.exe`,
      `${installerCleanup}-x86_64-pc-windows-msvc.exe`,
    ],
  );
  assert.deepEqual(
    sidecarFileNames(
      WINDOWS_SIDECAR_BINARIES,
      "x86_64-unknown-linux-gnu",
    ),
    [
      `${worker}-x86_64-unknown-linux-gnu`,
      `${installerCleanup}-x86_64-unknown-linux-gnu`,
    ],
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
    () => buildProfile([worker]),
    /unknown sidecar argument/,
  );
  assert.throws(
    () => targetDirectoryFromCargoMetadata("{}"),
    /target directory/,
  );
  assert.throws(
    () => targetDirectoryFromCargoMetadata("{invalid"),
    /cargo metadata output is not valid JSON/,
  );
});

test("reports missing sidecar output with a stable error", () => {
  assert.throws(
    () =>
      assertSidecarBuildOutput(
        path.join(process.cwd(), "missing-sidecar-output"),
        worker,
      ),
    /Windows sidecar build output is missing: hmm-save-backup-worker/,
  );
  assert.throws(
    () => assertSidecarBuildOutput(process.cwd(), installerCleanup),
    /Windows sidecar build output is missing: hmm-save-backup-installer-cleanup/,
  );
});

test("bounds captured command stderr diagnostics", () => {
  assert.equal(
    capturedCommandFailure("cargo", 17, "metadata failed\n").message,
    "cargo exited with 17: metadata failed",
  );
  const bounded = capturedCommandFailure("cargo", 1, "x".repeat(5_000));
  assert.ok(bounded.message.length < 4_100);
});

test("rejects unsafe or conflicting target triple input", () => {
  for (const target of ["", "../windows", "x86_64/windows", "x86_64\\windows"]) {
    assert.throws(
      () => sidecarFileName(worker, target),
      /invalid Rust target triple/,
    );
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

test("disables external binaries only for the inner Cargo build", () => {
  assert.deepEqual(JSON.parse(windowsSidecarBuildTauriConfig(undefined)), {
    bundle: { externalBin: [] },
  });

  const existingConfig = JSON.stringify({
    build: { devUrl: "http://localhost:1420" },
    bundle: {
      active: true,
      externalBin: [
        "binaries/hmm-save-backup-worker",
        "binaries/hmm-save-backup-installer-cleanup",
      ],
    },
  });
  assert.deepEqual(JSON.parse(windowsSidecarBuildTauriConfig(existingConfig)), {
    build: { devUrl: "http://localhost:1420" },
    bundle: { active: true, externalBin: [] },
  });
});

test("keeps the Windows bundle and prepare commands on the fixed inventory", () => {
  const packageJson = JSON.parse(
    readFileSync(path.join(process.cwd(), "package.json"), "utf8"),
  );
  assert.equal(
    packageJson.scripts["prepare:windows-sidecars"],
    "node scripts/prepare-windows-sidecars.mjs",
  );
  assert.equal(
    packageJson.scripts["prepare:windows-sidecars:dev"],
    "node scripts/prepare-windows-sidecars.mjs --debug",
  );
  assert.equal(packageJson.scripts["prepare:save-backup-worker-sidecar"], undefined);

  const windowsConfig = JSON.parse(
    readFileSync(
      path.join(process.cwd(), "src-tauri", "tauri.windows.conf.json"),
      "utf8",
    ),
  );
  assert.deepEqual(windowsConfig.bundle.externalBin, [
    "binaries/hmm-save-backup-worker",
    "binaries/hmm-save-backup-installer-cleanup",
  ]);
});

test("rejects invalid inner Cargo build Tauri configuration", () => {
  assert.throws(
    () => windowsSidecarBuildTauriConfig("{invalid"),
    /TAURI_CONFIG must be valid JSON/,
  );
  assert.throws(
    () => windowsSidecarBuildTauriConfig("[]"),
    /TAURI_CONFIG must be a JSON object/,
  );
  assert.throws(
    () => windowsSidecarBuildTauriConfig('{"bundle":[]}'),
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
