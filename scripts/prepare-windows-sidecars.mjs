import { spawnSync } from "node:child_process";
import { copyFileSync, lstatSync, mkdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

export const WINDOWS_SIDECAR_BINARIES = Object.freeze([
  "hmm-save-backup-worker",
  "hmm-save-backup-installer-cleanup",
]);

export function hostTripleFromRustc(output) {
  const match = /^host:\s+([^\s]+)$/m.exec(output);
  if (!match) {
    throw new Error("rustc host triple is unavailable");
  }
  return match[1];
}

export function targetDirectoryFromCargoMetadata(output) {
  let metadata;
  try {
    metadata = JSON.parse(output);
  } catch {
    throw new Error("cargo metadata output is not valid JSON");
  }
  if (
    typeof metadata.target_directory !== "string" ||
    metadata.target_directory.length === 0
  ) {
    throw new Error("Cargo target directory is unavailable");
  }
  return path.normalize(metadata.target_directory);
}

export function buildProfile(args) {
  if (args.length === 0) {
    return "release";
  }
  if (args.length === 1 && args[0] === "--debug") {
    return "debug";
  }
  throw new Error("unknown sidecar argument");
}

function assertSupportedBinaryName(binaryName) {
  if (!WINDOWS_SIDECAR_BINARIES.includes(binaryName)) {
    throw new Error("unsupported Windows sidecar binary");
  }
}

function assertTargetTriple(targetTriple) {
  if (!/^[A-Za-z0-9_.-]+$/.test(targetTriple)) {
    throw new Error("invalid Rust target triple");
  }
}

export function sidecarFileName(binaryName, targetTriple) {
  assertSupportedBinaryName(binaryName);
  assertTargetTriple(targetTriple);
  const extension = targetTriple.includes("windows") ? ".exe" : "";
  return `${binaryName}-${targetTriple}${extension}`;
}

export function sidecarFileNames(binaryNames, targetTriple) {
  const seen = new Set();
  return binaryNames.map((binaryName) => {
    assertSupportedBinaryName(binaryName);
    if (seen.has(binaryName)) {
      throw new Error("duplicate Windows sidecar binary");
    }
    seen.add(binaryName);
    return sidecarFileName(binaryName, targetTriple);
  });
}

export function resolveTargetTriple(explicitTarget, hostTarget, tauriArch) {
  const target = explicitTarget ?? hostTarget;
  assertTargetTriple(target);
  if (tauriArch && !target.startsWith(`${tauriArch}-`)) {
    throw new Error("sidecar target does not match TAURI_ENV_ARCH");
  }
  return target;
}

export function sidecarRustFlags(targetTriple, existingRustFlags) {
  if (!targetTriple.endsWith("windows-msvc")) {
    return existingRustFlags;
  }

  const staticCrtFlag = "-Ctarget-feature=+crt-static";
  return existingRustFlags
    ? `${existingRustFlags} ${staticCrtFlag}`
    : staticCrtFlag;
}

function isJsonObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function windowsSidecarBuildTauriConfig(existingConfig) {
  let config = {};
  if (existingConfig !== undefined) {
    try {
      config = JSON.parse(existingConfig);
    } catch {
      throw new Error("TAURI_CONFIG must be valid JSON");
    }
    if (!isJsonObject(config)) {
      throw new Error("TAURI_CONFIG must be a JSON object");
    }
  }

  const bundle = config.bundle ?? {};
  if (!isJsonObject(bundle)) {
    throw new Error("TAURI_CONFIG bundle must be a JSON object");
  }

  return JSON.stringify({
    ...config,
    bundle: {
      ...bundle,
      externalBin: [],
    },
  });
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    stdio: "inherit",
    ...options,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${command} exited with ${result.status}`);
  }
}

function capture(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
  });
  if (result.error || result.status !== 0) {
    throw (
      result.error ??
      capturedCommandFailure(command, result.status, result.stderr)
    );
  }
  return result.stdout ?? "";
}

export function capturedCommandFailure(command, status, stderr) {
  const detail =
    typeof stderr === "string" ? stderr.trim().slice(0, 4_000) : "";
  const suffix = detail.length > 0 ? `: ${detail}` : "";
  return new Error(`${command} exited with ${status}${suffix}`);
}

export function assertSidecarBuildOutput(source, binaryName) {
  assertSupportedBinaryName(binaryName);
  try {
    if (lstatSync(source).isFile()) {
      return;
    }
  } catch {
    // Normalize missing and unreadable build outputs to one stable build error.
  }
  throw new Error(`Windows sidecar build output is missing: ${binaryName}`);
}

export function assertNoDynamicMsvcCrtImports(
  binaryContents,
  binaryName,
  targetTriple,
) {
  assertSupportedBinaryName(binaryName);
  if (!targetTriple.endsWith("windows-msvc")) {
    return;
  }

  const imports = Buffer.from(binaryContents).toString("ascii").toUpperCase();
  const dynamicCrtImports = [
    "VCRUNTIME140.DLL",
    "VCRUNTIME140_1.DLL",
    "MSVCP140.DLL",
    "API-MS-WIN-CRT-RUNTIME-L1-1-0.DLL",
  ];
  if (dynamicCrtImports.some((dependency) => imports.includes(dependency))) {
    throw new Error(`Windows sidecar requires dynamic MSVC CRT: ${binaryName}`);
  }
}

function sidecarDestination(destinationDirectory, binaryName, targetTriple) {
  const resolvedDirectory = path.resolve(destinationDirectory);
  const destination = path.resolve(
    resolvedDirectory,
    sidecarFileName(binaryName, targetTriple),
  );
  if (path.dirname(destination) !== resolvedDirectory) {
    throw new Error("Windows sidecar destination escaped the binaries directory");
  }
  return destination;
}

export function prepareSidecars(args = []) {
  const profile = buildProfile(args);
  const hostTarget = hostTripleFromRustc(capture("rustc", ["-vV"]));
  const explicitTarget =
    process.env.HMM_SIDECAR_TARGET_TRIPLE ?? process.env.CARGO_BUILD_TARGET;
  const targetTriple = resolveTargetTriple(
    explicitTarget,
    hostTarget,
    process.env.TAURI_ENV_ARCH,
  );
  const cargoArgs = ["build", "-p", "hmm-save-backup-sidecars"];
  for (const binaryName of WINDOWS_SIDECAR_BINARIES) {
    cargoArgs.push("--bin", binaryName);
  }
  cargoArgs.push("--target", targetTriple);
  if (profile === "release") {
    cargoArgs.push("--release");
  }
  const cargoEnvironment = {
    ...process.env,
    TAURI_CONFIG: windowsSidecarBuildTauriConfig(process.env.TAURI_CONFIG),
  };
  const rustFlags = sidecarRustFlags(targetTriple, process.env.RUSTFLAGS);
  if (rustFlags) {
    cargoEnvironment.RUSTFLAGS = rustFlags;
  }
  run("cargo", cargoArgs, {
    env: cargoEnvironment,
  });

  const targetDirectory = targetDirectoryFromCargoMetadata(
    capture("cargo", ["metadata", "--format-version", "1", "--no-deps"]),
  );
  const extension = targetTriple.includes("windows") ? ".exe" : "";
  const destinationDirectory = path.join(repoRoot, "src-tauri", "binaries");
  mkdirSync(destinationDirectory, { recursive: true });

  for (const binaryName of WINDOWS_SIDECAR_BINARIES) {
    const source = path.join(
      targetDirectory,
      targetTriple,
      profile,
      `${binaryName}${extension}`,
    );
    assertSidecarBuildOutput(source, binaryName);
    assertNoDynamicMsvcCrtImports(
      readFileSync(source),
      binaryName,
      targetTriple,
    );
    copyFileSync(
      source,
      sidecarDestination(destinationDirectory, binaryName, targetTriple),
    );
  }
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
) {
  prepareSidecars(process.argv.slice(2));
}
