import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const scriptsDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(scriptsDir, "..");

function readRepoFile(relativePath) {
  return readFileSync(join(repoRoot, ...relativePath.split("/")), "utf8");
}

function assertOrdered(content, excerpts, label) {
  let cursor = 0;
  for (const excerpt of excerpts) {
    const index = content.indexOf(excerpt, cursor);
    assert.notEqual(index, -1, `${label} must contain ${excerpt} after offset ${cursor}`);
    cursor = index + excerpt.length;
  }
}

test("PowerShell verification runs the full quality sequence and fails closed", () => {
  const script = readRepoFile("scripts/verify.ps1");

  assert.match(
    script,
    /function Invoke-Pnpm[\s\S]*?if \(\$LASTEXITCODE -ne 0\) \{\s*exit \$LASTEXITCODE\s*\}/,
    "Invoke-Pnpm must propagate a failing pnpm exit code",
  );
  assertOrdered(
    script,
    [
      "node --test scripts/verify-entrypoints.test.mjs",
      'Invoke-Pnpm -Arguments @("run", "typecheck")',
      'Invoke-Pnpm -Arguments @("run", "lint")',
      'Invoke-Pnpm -Arguments @("run", "test")',
      'Invoke-Pnpm -Arguments @("run", "build")',
      "cargo test --workspace",
      "cargo check --workspace",
      "cargo clippy --workspace --all-targets -- -D warnings",
    ],
    "scripts/verify.ps1",
  );
  assert.match(
    script,
    /node --test scripts\/verify-entrypoints\.test\.mjs\s*if \(\$LASTEXITCODE -ne 0\) \{\s*exit \$LASTEXITCODE\s*\}/,
    "verification contract test failure must stop the PowerShell entrypoint",
  );
  for (const command of [
    "cargo test --workspace",
    "cargo check --workspace",
    "cargo clippy --workspace --all-targets -- -D warnings",
  ]) {
    const escaped = command.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    assert.match(
      script,
      new RegExp(
        `${escaped}\\s*if \\(\\$LASTEXITCODE -ne 0\\) \\{\\s*exit \\$LASTEXITCODE\\s*\\}`,
      ),
      `${command} must propagate a failing exit code`,
    );
  }
});

test("Bash verification matches the PowerShell quality sequence and fails closed", () => {
  const script = readRepoFile("scripts/verify.sh");

  assert.match(script, /^set -euo pipefail$/m, "Bash entrypoint must fail on command errors");
  assertOrdered(
    script,
    [
      '"${node_bin}" --test scripts/verify-entrypoints.test.mjs',
      "invoke_pnpm run typecheck",
      "invoke_pnpm run lint",
      "invoke_pnpm run test",
      "invoke_pnpm run build",
      "cargo test --workspace",
      "cargo check --workspace",
      "cargo clippy --workspace --all-targets -- -D warnings",
    ],
    "scripts/verify.sh",
  );
});

test("required CI context delegates to the full Bash verification entrypoint", () => {
  const workflow = readRepoFile(".github/workflows/verify.yml");

  assert.match(workflow, /^\s+name: Policy and docs$/m);
  assert.match(
    workflow,
    /- name: Run full verification \(includes frontend tests and Rust clippy\)[\s\S]*?run: bash scripts\/verify\.sh/,
  );
});
