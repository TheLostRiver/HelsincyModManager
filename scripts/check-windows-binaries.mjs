import { lstatSync, readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { WINDOWS_SIDECAR_BINARIES } from "./prepare-windows-sidecars.mjs";
import { inspectWindowsPe, isDynamicMsvcCrtImport } from "./windows-pe-imports.mjs";

export const WINDOWS_BUNDLE_BINARIES = Object.freeze([
  "hmm-tauri",
  ...WINDOWS_SIDECAR_BINARIES,
]);

export function checkWindowsBinaries(directory, { debug = false } = {}) {
  const results = [];
  for (const name of WINDOWS_BUNDLE_BINARIES) {
    const binary = path.join(directory, `${name}.exe`);
    let contents;
    try {
      if (!lstatSync(binary).isFile()) throw new Error("not a regular file");
      contents = readFileSync(binary);
    } catch {
      throw new Error(`Windows bundle binary is missing or not a regular file: ${name}`);
    }
    let pe;
    try {
      pe = inspectWindowsPe(contents);
    } catch {
      throw new Error(`Windows bundle binary has an invalid PE: ${name}`);
    }
    // Windows x64 is the supported release target; reject mixed/wrong artifacts.
    if (pe.machine !== 0x8664 || pe.format !== "PE32+") {
      throw new Error(`Windows bundle binary must be x64: ${name}`);
    }
    const allowedSubsystems = debug && name === "hmm-tauri" ? [2, 3] : [2];
    if (!allowedSubsystems.includes(pe.subsystem)) {
      throw new Error(`Windows bundle binary has an unexpected subsystem: ${name}`);
    }
    const dynamicCrt = pe.imports.filter(isDynamicMsvcCrtImport);
    if (dynamicCrt.length !== 0) {
      throw new Error(`Windows bundle binary requires dynamic MSVC CRT: ${name} (${dynamicCrt.join(", ")})`);
    }
    results.push({ name, subsystem: pe.subsystem, imports: pe.imports });
  }
  return results;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const [directory, flag, ...extra] = process.argv.slice(2);
  if (!directory || directory.startsWith("--") || (flag && flag !== "--debug") || extra.length !== 0) {
    throw new Error("Usage: node scripts/check-windows-binaries.mjs <artifact-directory> [--debug]");
  }
  const results = checkWindowsBinaries(directory, { debug: flag === "--debug" });
  for (const { name, subsystem } of results) {
    console.log(`${name}: x64 PE, subsystem=${subsystem}, no dynamic MSVC CRT imports`);
  }
}
