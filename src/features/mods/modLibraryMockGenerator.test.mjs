import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "../../..");
const scriptPath = join(repoRoot, "scripts", "generate-mod-library-mock-data.mjs");
const sourceFilePath = join(repoRoot, "src", "features", "mods", "modsLibraryData.ts");

function createTempOutputPath() {
  const tempDir = mkdtempSync(join(tmpdir(), "hmm-mod-mock-"));
  return {
    tempDir,
    outputPath: join(tempDir, "modsLibraryData.generated.ts"),
  };
}

function runGenerator(args = []) {
  const { tempDir, outputPath } = createTempOutputPath();

  try {
    execFileSync(process.execPath, [scriptPath, "--output", outputPath, ...args], {
      cwd: repoRoot,
      encoding: "utf8",
    });

    return {
      outputPath,
      content: readFileSync(outputPath, "utf8"),
    };
  } finally {
    rmSync(tempDir, { recursive: true, force: true });
  }
}

function runGeneratorExpectError(args = []) {
  try {
    runGenerator(args);
    assert.fail("Expected generator to throw");
  } catch (error) {
    return error;
  }
}

function parseModItemIds(content) {
  const modItemsBlockMatch = content.match(/export const modLibraryItems: ModLibraryItem\[] = \[([\s\S]*?)\n\];/);
  assert.ok(modItemsBlockMatch, "Expected modLibraryItems array block");

  return [...modItemsBlockMatch[1].matchAll(/id:\s*"([^"]+)"/g)].map((match) => match[1]);
}

function parseModItemCount(content) {
  return parseModItemIds(content).length;
}

test("mock generator defaults to 10 items", () => {
  const { content } = runGenerator();

  assert.equal(parseModItemCount(content), 10);
});

test("mock generator accepts --count within 1-999", () => {
  const direct = runGenerator(["--count", "72"]);
  const withSeparator = runGenerator(["--", "--count", "72"]);

  assert.equal(parseModItemCount(direct.content), 72);
  assert.equal(parseModItemCount(withSeparator.content), 72);
});

test("mock generator rejects invalid count values", () => {
  for (const count of ["0", "-1", "1000", "abc", "12.5"]) {
    const error = runGeneratorExpectError(["--count", count]);
    const stderr = error.stderr?.toString?.() ?? "";
    const stdout = error.stdout?.toString?.() ?? "";
    const combinedOutput = `${stdout}\n${stderr}`;

    assert.match(combinedOutput, /count/i, `Expected count validation error for ${count}`);
  }
});

test("mock generator keeps ids unique", () => {
  const { content } = runGenerator(["--count", "72"]);
  const ids = parseModItemIds(content);

  assert.equal(ids.length, 72);
  assert.equal(new Set(ids).size, 72);
});

test("mock generator covers every visible development category", () => {
  const { content } = runGenerator(["--count", "72"]);

  for (const category of ["外观", "防具替换", "武器替换", "语音替换", "工具 / 前置"]) {
    assert.ok(content.includes(`{ name: ${JSON.stringify(category)} }`), `Expected generated category ${category}`);
  }
});

test("mock generator reuses the canonical ModLibraryItem contract", () => {
  const { content } = runGenerator(["--count", "1"]);

  assert.match(content, /import type \{ ModLibraryItem \} from "\.\/modLibraryTypes";/);
  assert.match(content, /export type \{ ModInstallStatus, ModLibraryItem \} from "\.\/modLibraryTypes";/);
  assert.doesNotMatch(content, /export type ModLibraryItem\s*=/);
  assert.doesNotMatch(content, /categoryLabels:\s*string\[\]/);
});

test("mock generator preserves separate import, install and true reinstall actions", () => {
  const { content } = runGenerator(["--count", "1"]);

  for (const actionId of ["add", "add-revision", "preview-plan", "install", "reinstall", "uninstall", "delete"]) {
    assert.match(content, new RegExp(`id: "${actionId}"`));
  }
  assert.doesNotMatch(content, /安装 \/ 重装选中 MOD/);
});

test("committed modsLibraryData matches the deterministic 72-item output", () => {
  const sourceContent = readFileSync(sourceFilePath, "utf8");
  const { content } = runGenerator(["--count", "72"]);

  assert.equal(parseModItemCount(sourceContent), 72);
  assert.equal(sourceContent, content);
});
