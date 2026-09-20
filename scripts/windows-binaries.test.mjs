import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { checkWindowsBinaries, WINDOWS_BUNDLE_BINARIES } from "./check-windows-binaries.mjs";
import { inspectWindowsPe, isDynamicMsvcCrtImport } from "./windows-pe-imports.mjs";

const repoRoot = fileURLToPath(new URL("../", import.meta.url));
const PE = 0x80;
const OPTIONAL = PE + 24;
const RAW = 0x200;
const RVA = 0x1000;
const DELAY = 0x380;
const NAMES = 0x500;

// Synthetic headers and import descriptors only; never executable test programs.
function fixture({ pe32 = false, imports = [], delayed = [], legacy = false } = {}) {
  const bytes = Buffer.alloc(0xa00);
  const directoryOffset = pe32 ? 96 : 112;
  const optionalSize = directoryOffset + 16 * 8;
  bytes.writeUInt16LE(0x5a4d, 0);
  bytes.writeUInt32LE(PE, 0x3c);
  bytes.writeUInt32LE(0x4550, PE);
  bytes.writeUInt16LE(pe32 ? 0x14c : 0x8664, PE + 4);
  bytes.writeUInt16LE(1, PE + 6);
  bytes.writeUInt16LE(optionalSize, PE + 20);
  bytes.writeUInt16LE(pe32 ? 0x10b : 0x20b, OPTIONAL);
  if (pe32) bytes.writeUInt32LE(0x400000, OPTIONAL + 28);
  else bytes.writeBigUInt64LE(0x140000000n, OPTIONAL + 24);
  bytes.writeUInt32LE(RAW, OPTIONAL + 60);
  bytes.writeUInt16LE(2, OPTIONAL + 68);
  bytes.writeUInt32LE(16, OPTIONAL + directoryOffset - 4);
  const section = OPTIONAL + optionalSize;
  bytes.writeUInt32LE(0x800, section + 8);
  bytes.writeUInt32LE(RVA, section + 12);
  bytes.writeUInt32LE(0x800, section + 16);
  bytes.writeUInt32LE(RAW, section + 20);
  let nextName = NAMES;
  for (const [names, index, start, stride, nameOffset] of [
    [imports, 1, RAW, 20, 12],
    [delayed, 13, DELAY, 32, 4],
  ]) {
    if (names.length === 0) continue;
    const dir = OPTIONAL + directoryOffset + index * 8;
    bytes.writeUInt32LE(RVA + start - RAW, dir);
    bytes.writeUInt32LE((names.length + 1) * stride, dir + 4);
    names.forEach((name, entry) => {
      const descriptor = start + entry * stride;
      if (index === 13) bytes.writeUInt32LE(legacy ? 0 : 1, descriptor);
      bytes.writeUInt32LE(
        RVA + nextName - RAW + (index === 13 && legacy ? 0x400000 : 0),
        descriptor + nameOffset,
      );
      nextName += bytes.write(`${name}\0`, nextName, "ascii");
    });
  }
  return bytes;
}

const dynamicNames = [
  "MSVCP140.dll", "msvcp140_atomic_wait.dll", "MSVCP140D.dll", "MSVCR120.dll",
  "VCRUNTIME140.dll", "vcruntime140_1.dll", "VCRUNTIME140_1D.DLL",
  "CONCRT140.dll", "vcomp140.dll", "vccorlib140.dll", "ucrtbase.dll", "ucrtbased.dll",
  "api-ms-win-crt-runtime-l1-1-0.dll", "API-MS-WIN-CRT-MATH-L1-1-0.DLL",
];

test("reads PE32/PE32+ ordinary and delay imports without scanning unrelated strings", () => {
  for (const pe32 of [false, true]) {
    const bytes = fixture({ pe32, imports: ["KERNEL32.dll", "USER32.dll"], delayed: ["VERSION.dll"] });
    bytes.write("MSVCP140.dll\0", 0x850, "ascii");
    assert.deepEqual(inspectWindowsPe(bytes), {
      machine: pe32 ? 0x14c : 0x8664,
      format: pe32 ? "PE32" : "PE32+",
      subsystem: 2,
      imports: ["KERNEL32.dll", "USER32.dll", "VERSION.dll"],
    });
    assert.deepEqual(inspectWindowsPe(fixture({ pe32 })).imports, []);
  }
  assert.deepEqual(inspectWindowsPe(fixture({ pe32: true, legacy: true, delayed: ["MSVCP140.dll"] })).imports,
    ["MSVCP140.dll"]);
});

test("recognizes dynamic CRT families, case and debug/numbered variants", () => {
  for (const name of dynamicNames) assert.equal(isDynamicMsvcCrtImport(name), true, name);
  for (const name of ["KERNEL32.dll", "USER32.dll", "msvcrt.dll", "msvcp_win.dll", "api-ms-win-core-synch-l1-2-0.dll"]) {
    assert.equal(isDynamicMsvcCrtImport(name), false, name);
  }
});

test("rejects truncated headers, sections and ordinary/delay import directories", () => {
  const bytes = fixture({ imports: ["KERNEL32.dll"], delayed: ["VERSION.dll"] });
  for (let size = 0; size < bytes.length; size += 1) {
    assert.throws(() => inspectWindowsPe(bytes.subarray(0, size)), /Invalid Windows PE binary/, `size=${size}`);
  }
});

test("rejects malformed PE offsets, descriptors and DLL names instead of passing an empty list", () => {
  const dir = OPTIONAL + 112;
  const section = dir + 16 * 8;
  const mutations = [
    b => b.writeUInt16LE(0, 0),
    b => b.writeUInt32LE(32, 0x3c),
    b => b.writeUInt32LE(0xffffffff, 0x3c),
    b => b.writeUInt32LE(0, PE),
    b => b.writeUInt16LE(0, PE + 6),
    b => b.writeUInt16LE(97, PE + 6),
    b => b.writeUInt16LE(1, PE + 20),
    b => b.writeUInt16LE(0xffff, PE + 20),
    b => b.writeUInt16LE(0, OPTIONAL),
    b => b.writeUInt32LE(17, dir - 4),
    b => b.writeUInt32LE(16, OPTIONAL + 60),
    b => b.writeUInt32LE(0xffff, OPTIONAL + 60),
    b => b.writeUInt32LE(0xffff, section + 16),
    b => b.writeUInt32LE(1, section + 20),
    b => b.writeUInt32LE(1, section + 12),
    b => b.writeUInt32LE(0, dir + 8),
    b => b.writeUInt32LE(0, dir + 12),
    b => b.writeUInt32LE(19, dir + 12),
    b => b.writeUInt32LE(0xffff, dir + 12),
    b => b.writeUInt32LE(0xffff, dir + 8),
    b => b.writeUInt32LE(20, dir + 12), // No zero descriptor before directory ends.
    b => { b.writeUInt32LE(1, RAW); b.writeUInt32LE(0, RAW + 12); },
    b => b.writeUInt32LE(0xffff, RAW + 12),
    b => b.fill(0x41, NAMES), // Name has no bounded NUL terminator.
    b => b.writeUInt8(0xff, NAMES),
    b => b.writeUInt8(0x20, NAMES),
    b => b.writeUInt8(0, NAMES),
    b => b.writeUInt32LE(31, dir + 13 * 8 + 4),
    b => b.writeUInt32LE(32, dir + 13 * 8 + 4),
    b => b.writeUInt32LE(2, DELAY), // Unknown delay attributes.
    b => b.writeUInt32LE(0, DELAY), // PE32+ legacy VA below its image base.
    b => { // Two sections claim the same import RVA.
      b.writeUInt16LE(2, PE + 6);
      b.copy(b, section + 40, section, section + 40);
    },
    b => { // Import name points at zero-filled virtual tail, absent from the file.
      b.writeUInt32LE(0x1000, section + 8);
      b.writeUInt32LE(RVA + 0x900, RAW + 12);
    },
  ];
  for (const [index, mutate] of mutations.entries()) {
    const bytes = fixture({ imports: ["KERNEL32.dll"], delayed: ["VERSION.dll"] });
    mutate(bytes);
    assert.throws(() => inspectWindowsPe(bytes), /Invalid Windows PE binary/, `mutation=${index}`);
  }
});

function bundleDirectory(t) {
  const directory = mkdtempSync(path.join(tmpdir(), "hmm-pe-test-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  for (const name of WINDOWS_BUNDLE_BINARIES) {
    writeFileSync(path.join(directory, `${name}.exe`), fixture({ imports: ["KERNEL32.dll"] }));
  }
  return directory;
}

test("checks the GUI and exactly both shipped helpers, and requires regular files", t => {
  assert.deepEqual(WINDOWS_BUNDLE_BINARIES, [
    "hmm-tauri", "hmm-save-backup-worker", "hmm-save-backup-installer-cleanup",
  ]);
  const directory = bundleDirectory(t);
  assert.deepEqual(checkWindowsBinaries(directory).map(result => result.name), WINDOWS_BUNDLE_BINARIES);
  for (const name of WINDOWS_BUNDLE_BINARIES) {
    const file = path.join(directory, `${name}.exe`);
    rmSync(file);
    assert.throws(() => checkWindowsBinaries(directory), /missing or not a regular file/);
    mkdirSync(file);
    assert.throws(() => checkWindowsBinaries(directory), /missing or not a regular file/);
    rmSync(file, { recursive: true });
    writeFileSync(file, fixture());
  }
});

test("each bundle binary rejects malformed PE, wrong machine and non-GUI release subsystems", t => {
  const directory = bundleDirectory(t);
  for (const name of WINDOWS_BUNDLE_BINARIES) {
    const file = path.join(directory, `${name}.exe`);
    writeFileSync(file, "not a PE");
    assert.throws(() => checkWindowsBinaries(directory), /invalid PE/);
    writeFileSync(file, fixture({ pe32: true }));
    assert.throws(() => checkWindowsBinaries(directory), /must be x64/);
    const mismatched = fixture({ pe32: true });
    mismatched.writeUInt16LE(0x8664, PE + 4);
    writeFileSync(file, mismatched);
    assert.throws(() => checkWindowsBinaries(directory), /must be x64/);
    for (const subsystem of [0, 1, 3, 7]) {
      const bytes = fixture();
      bytes.writeUInt16LE(subsystem, OPTIONAL + 68);
      writeFileSync(file, bytes);
      assert.throws(() => checkWindowsBinaries(directory), /unexpected subsystem/);
      if (name === "hmm-tauri" && subsystem === 3) checkWindowsBinaries(directory, { debug: true });
      else assert.throws(() => checkWindowsBinaries(directory, { debug: true }), /unexpected subsystem/);
    }
    writeFileSync(file, fixture());
  }
});

test("all three binaries reject every dynamic CRT variant in ordinary and delay imports", t => {
  const directory = bundleDirectory(t);
  for (const name of WINDOWS_BUNDLE_BINARIES) {
    const file = path.join(directory, `${name}.exe`);
    for (const key of ["imports", "delayed"]) {
      for (const dependency of dynamicNames) {
        writeFileSync(file, fixture({ [key]: [dependency] }));
        assert.throws(() => checkWindowsBinaries(directory), /requires dynamic MSVC CRT/);
      }
    }
    writeFileSync(file, fixture());
  }
});

test("CLI exits successfully only for valid artifacts and explicit supported arguments", t => {
  const directory = bundleDirectory(t);
  const cli = (...args) => spawnSync(process.execPath, [path.join(repoRoot, "scripts/check-windows-binaries.mjs"), ...args],
    { encoding: "utf8", windowsHide: true });
  assert.equal(cli(directory).status, 0);
  assert.equal(cli(directory, "--debug").status, 0);
  for (const args of [[], ["--debug"], [directory, "--unknown"], [directory, "--debug", "extra"]]) {
    const result = cli(...args);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /Usage:/);
  }
  writeFileSync(path.join(directory, "hmm-tauri.exe"), fixture({ imports: ["MSVCP140.dll"] }));
  const result = cli(directory);
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /requires dynamic MSVC CRT: hmm-tauri \(MSVCP140.dll\)/);
});

test("repository CRT configuration covers every Windows MSVC target, not other platforms", () => {
  const config = readFileSync(path.join(repoRoot, ".cargo/config.toml"), "utf8")
    .split(/\r?\n/).map(line => line.replace(/#.*/, "").trim()).filter(Boolean);
  assert.deepEqual(config, [
    `[target.'cfg(all(target_os = "windows", target_env = "msvc"))']`,
    'rustflags = ["-Ctarget-feature=+crt-static"]',
    "[env]",
    'STATIC_VCRUNTIME = { value = "false", force = true }',
  ]);
  const build = readFileSync(path.join(repoRoot, "src-tauri/build.rs"), "utf8");
  assert.match(build, /^\s*println!\("cargo:rerun-if-env-changed=STATIC_VCRUNTIME"\);$/m);
});
