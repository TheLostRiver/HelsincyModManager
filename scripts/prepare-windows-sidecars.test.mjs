import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  WINDOWS_SIDECAR_BINARIES,
  assertNoDynamicMsvcCrtImports,
  assertSidecarBuildOutput,
  assertWindowsGuiSubsystem,
  buildProfile,
  capturedCommandFailure,
  copySidecarBuildOutput,
  hostTripleFromRustc,
  resolveTargetTriple,
  sidecarRustFlags,
  sidecarFileName,
  sidecarFileNames,
  targetDirectoryFromCargoMetadata,
  windowsSidecarBuildTauriConfig,
} from "./prepare-windows-sidecars.mjs";

const worker = "hmm-save-backup-worker";
const installerCleanup = "hmm-save-backup-installer-cleanup";

function peHeaderFixture({ magic = 0x20b, subsystem = 2, peOffset = 0x80 } = {}) {
  const optionalSize = magic === 0x10b ? 224 : 240;
  const bytes = Buffer.alloc(peOffset + 24 + optionalSize);
  bytes.writeUInt16LE(0x5a4d, 0);
  bytes.writeUInt32LE(peOffset, 0x3c);
  bytes.writeUInt32LE(0x00004550, peOffset);
  bytes.writeUInt16LE(magic === 0x10b ? 0x14c : 0x8664, peOffset + 4);
  bytes.writeUInt16LE(optionalSize, peOffset + 20);
  bytes.writeUInt16LE(magic, peOffset + 24);
  bytes.writeUInt16LE(subsystem, peOffset + 24 + 68);
  return bytes;
}

function temporarySidecarDirectory(t) {
  const directory = mkdtempSync(path.join(tmpdir(), "hmm-sidecar-test-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  return directory;
}

test("accepts Windows GUI subsystem in PE32 and PE32+ for both sidecars", () => {
  for (const binary of WINDOWS_SIDECAR_BINARIES) {
    for (const magic of [0x10b, 0x20b]) {
      for (const peOffset of [0x40, 0x80, 0x100]) {
        assert.doesNotThrow(() =>
          assertWindowsGuiSubsystem(
            peHeaderFixture({ magic, peOffset }), binary, "x86_64-pc-windows-msvc",
          ),
        );
      }
    }
  }
});

test("rejects console and every other tested non-GUI Windows subsystem", () => {
  for (const binary of WINDOWS_SIDECAR_BINARIES) {
    for (const target of [
      "x86_64-pc-windows-msvc", "x86_64-pc-windows-gnu", "aarch64-pc-windows-msvc",
    ]) {
      for (const magic of [0x10b, 0x20b]) {
        for (const subsystem of [0, 1, 3, 7, 9, 10, 0xffff]) {
          assert.throws(
            () => assertWindowsGuiSubsystem(
              peHeaderFixture({ magic, subsystem }), binary, target,
            ),
            { message: `Windows sidecar must use the Windows GUI subsystem: ${binary}` },
          );
        }
      }
    }
  }
});

test("rejects malformed or truncated PE headers with a stable error", () => {
  const malformed = [
    Buffer.alloc(0), Buffer.alloc(63), Buffer.from("not a PE file"),
    peHeaderFixture({ magic: 0x107 }),
  ];
  for (const mutate of [
    (bytes) => bytes.writeUInt16LE(0, 0),
    (bytes) => bytes.writeUInt32LE(0, 0x80),
    (bytes) => bytes.writeUInt32LE(63, 0x3c),
    (bytes) => bytes.writeUInt32LE(0xffffffff, 0x3c),
    (bytes) => bytes.writeUInt32LE(bytes.length - 23, 0x3c),
    (bytes) => bytes.writeUInt16LE(0, 0x80 + 20),
    (bytes) => bytes.writeUInt16LE(1, 0x80 + 20),
    (bytes) => bytes.writeUInt16LE(69, 0x80 + 20),
    (bytes) => bytes.writeUInt16LE(111, 0x80 + 20),
    (bytes) => bytes.writeUInt16LE(0xffff, 0x80 + 20),
  ]) {
    const bytes = peHeaderFixture();
    mutate(bytes);
    malformed.push(bytes);
  }
  const shortPe32 = peHeaderFixture({ magic: 0x10b });
  shortPe32.writeUInt16LE(95, 0x80 + 20);
  malformed.push(shortPe32);
  for (const magic of [0x10b, 0x20b]) {
    const valid = peHeaderFixture({ magic });
    for (let length = 0; length < valid.length; length += 1) {
      malformed.push(valid.subarray(0, length));
    }
  }
  for (const bytes of malformed) {
    assert.throws(
      () => assertWindowsGuiSubsystem(bytes, worker, "x86_64-pc-windows-msvc"),
      { message: `Windows sidecar has an invalid PE header: ${worker}` },
    );
  }
});

test("does not apply PE requirements to non-Windows outputs", () => {
  for (const target of ["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"]) {
    assert.doesNotThrow(() =>
      assertWindowsGuiSubsystem(Buffer.from("non-PE fixture"), worker, target),
    );
  }
  assert.throws(
    () => assertWindowsGuiSubsystem(
      peHeaderFixture(), "arbitrary-helper", "x86_64-pc-windows-msvc",
    ),
    /unsupported Windows sidecar binary/,
  );
  assert.throws(
    () => assertWindowsGuiSubsystem(peHeaderFixture(), worker, "../windows"),
    /invalid Rust target triple/,
  );
});

test("copies verified GUI sidecars to their exact bundle input names", (t) => {
  const directory = temporarySidecarDirectory(t);
  const destinationDirectory = path.join(directory, "binaries");
  mkdirSync(destinationDirectory);
  for (const binary of WINDOWS_SIDECAR_BINARIES) {
    const source = path.join(directory, `${binary}.exe`);
    const bytes = peHeaderFixture();
    writeFileSync(source, bytes);
    const destination = copySidecarBuildOutput(
      source, destinationDirectory, binary, "x86_64-pc-windows-msvc",
    );
    assert.equal(
      destination, path.join(destinationDirectory, `${binary}-x86_64-pc-windows-msvc.exe`),
    );
    assert.deepEqual(readFileSync(destination), bytes);
  }
});

test("invalid build outputs cannot create or overwrite a bundle input", (t) => {
  const directory = temporarySidecarDirectory(t);
  const destinationDirectory = path.join(directory, "binaries");
  mkdirSync(destinationDirectory);
  for (const binary of WINDOWS_SIDECAR_BINARIES) {
    const source = path.join(directory, `${binary}.exe`);
    const destination = path.join(destinationDirectory, `${binary}-x86_64-pc-windows-msvc.exe`);
    for (const bytes of [
      peHeaderFixture({ subsystem: 3 }),
      Buffer.alloc(0),
      Buffer.concat([peHeaderFixture(), Buffer.from("VCRUNTIME140.dll")]),
    ]) {
      writeFileSync(source, bytes);
      assert.throws(() => copySidecarBuildOutput(
        source, destinationDirectory, binary, "x86_64-pc-windows-msvc",
      ));
      assert.equal(existsSync(destination), false);
    }
    const previous = peHeaderFixture();
    writeFileSync(destination, previous);
    writeFileSync(source, peHeaderFixture({ subsystem: 3 }));
    assert.throws(
      () => copySidecarBuildOutput(
        source, destinationDirectory, binary, "x86_64-pc-windows-msvc",
      ),
      /must use the Windows GUI subsystem/,
    );
    assert.deepEqual(readFileSync(destination), previous);
  }
});

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

test("statically links the CRT for MSVC Windows sidecars", () => {
  assert.equal(
    sidecarRustFlags("x86_64-pc-windows-msvc", undefined),
    "-Ctarget-feature=+crt-static",
  );
  assert.equal(
    sidecarRustFlags("x86_64-pc-windows-msvc", "-Dwarnings"),
    "-Dwarnings -Ctarget-feature=+crt-static",
  );
  assert.equal(
    sidecarRustFlags("x86_64-pc-windows-gnu", "-Dwarnings"),
    "-Dwarnings",
  );
  assert.equal(
    sidecarRustFlags("x86_64-unknown-linux-gnu", undefined),
    undefined,
  );
});

test("rejects dynamic MSVC CRT imports before copying sidecars", () => {
  assert.doesNotThrow(() =>
    assertNoDynamicMsvcCrtImports(
      Buffer.from("static fixture"),
      installerCleanup,
      "x86_64-pc-windows-msvc",
    ),
  );
  assert.throws(
    () =>
      assertNoDynamicMsvcCrtImports(
        Buffer.from("fixture VCRUNTIME140.dll import"),
        installerCleanup,
        "x86_64-pc-windows-msvc",
      ),
    /Windows sidecar requires dynamic MSVC CRT: hmm-save-backup-installer-cleanup/,
  );
  assert.doesNotThrow(() =>
    assertNoDynamicMsvcCrtImports(
      Buffer.from("fixture VCRUNTIME140.dll import"),
      worker,
      "x86_64-pc-windows-gnu",
    ),
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
