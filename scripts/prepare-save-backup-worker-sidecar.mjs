import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

export function hostTripleFromRustc(output) {
  const match = /^host:\s+([^\s]+)$/m.exec(output);
  if (!match) {
    throw new Error("rustc host triple is unavailable");
  }
  return match[1];
}

export function targetDirectoryFromCargoMetadata(output) {
  const metadata = JSON.parse(output);
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

export function sidecarFileName(targetTriple) {
  if (!/^[A-Za-z0-9_.-]+$/.test(targetTriple)) {
    throw new Error("invalid Rust target triple");
  }
  const extension = targetTriple.includes("windows") ? ".exe" : "";
  return `hmm-save-backup-worker-${targetTriple}${extension}`;
}

export function resolveTargetTriple(explicitTarget, hostTarget, tauriArch) {
  const target = explicitTarget ?? hostTarget;
  sidecarFileName(target);
  if (tauriArch && !target.startsWith(`${tauriArch}-`)) {
    throw new Error("sidecar target does not match TAURI_ENV_ARCH");
  }
  return target;
}

function isJsonObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function workerBuildTauriConfig(existingConfig) {
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
    throw result.error ?? new Error(`${command} exited with ${result.status}`);
  }
  return result.stdout ?? "";
}

export function prepareSidecar(args = []) {
  const profile = buildProfile(args);
  const hostTarget = hostTripleFromRustc(capture("rustc", ["-vV"]));
  const explicitTarget =
    process.env.HMM_SIDECAR_TARGET_TRIPLE ?? process.env.CARGO_BUILD_TARGET;
  const targetTriple = resolveTargetTriple(
    explicitTarget,
    hostTarget,
    process.env.TAURI_ENV_ARCH,
  );
  const cargoArgs = [
    "build",
    "-p",
    "hmm-tauri",
    "--bin",
    "hmm-save-backup-worker",
    "--target",
    targetTriple,
  ];
  if (profile === "release") {
    cargoArgs.push("--release");
  }
  run("cargo", cargoArgs, {
    env: {
      ...process.env,
      TAURI_CONFIG: workerBuildTauriConfig(process.env.TAURI_CONFIG),
    },
  });

  const targetDirectory = targetDirectoryFromCargoMetadata(
    capture("cargo", ["metadata", "--format-version", "1", "--no-deps"]),
  );
  const extension = targetTriple.includes("windows") ? ".exe" : "";
  const source = path.join(
    targetDirectory,
    targetTriple,
    profile,
    `hmm-save-backup-worker${extension}`,
  );
  if (!statSync(source).isFile()) {
    throw new Error("worker sidecar build output is missing");
  }

  const destinationDirectory = path.join(repoRoot, "src-tauri", "binaries");
  mkdirSync(destinationDirectory, { recursive: true });
  copyFileSync(
    source,
    path.join(destinationDirectory, sidecarFileName(targetTriple)),
  );
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
) {
  prepareSidecar(process.argv.slice(2));
}
