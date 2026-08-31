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
  const invokePnpmStart = script.indexOf("function Invoke-Pnpm");
  const invokePnpmEnd = script.indexOf("function Assert-RequiredFile");

  assert.notEqual(invokePnpmStart, -1, "scripts/verify.ps1 must define Invoke-Pnpm");
  assert.ok(
    invokePnpmEnd > invokePnpmStart,
    "Invoke-Pnpm must end before Assert-RequiredFile",
  );
  const invokePnpm = script.slice(invokePnpmStart, invokePnpmEnd);

  assert.match(
    invokePnpm,
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

/*
 * 策略检查在这个仓库里有**两套独立实现**：
 *
 * - Windows 本地（`scripts/verify.ps1`）：依次执行一组 PowerShell 检查器；
 * - CI / Linux（`scripts/verify.sh`）：执行单一的 `check-policy.mjs --scope verify`。
 *
 * 它们今天对同一个问题会给出同样的结论（实测：同一个坏链接两边报同样的错），
 * 但没有任何机制保证以后不漂移。一旦漂移，表现就是「本地全过、CI 红」——
 * 而排查成本极高，因为本地根本复现不出来。
 *
 * 下面这个矩阵把两边钉在一起：**新增策略检查必须两边都登记**，否则用例红。
 */

const POLICY_CHECK_MATRIX = [
  { check: "Policy check", ps1: "scripts/check-policy.ps1" },
  { check: "Whitespace check", ps1: "scripts/check-whitespace.ps1" },
  { check: "File size check", ps1: "scripts/check-file-size.ps1" },
  { check: "Forbidden files check", ps1: "scripts/check-forbidden-files.ps1" },
  { check: "Markdown link check", ps1: "scripts/check-doc-links.ps1" },
  { check: "Frontend boundary check", ps1: "scripts/check-frontend-boundaries.ps1" },
  { check: "Secret scan", ps1: "scripts/check-secrets.ps1" },
];

function stripOutputLines(script) {
  return script
    .split("\n")
    .filter((line) => !/^\s*(Write-Host|echo)\b/.test(line))
    .join("\n");
}

function readPowerShellCheckList(script) {
  const listStart = script.indexOf("$checks = @(");
  assert.notEqual(listStart, -1, "scripts/verify.ps1 must declare a $checks list");
  const listEnd = script.indexOf(")", listStart);
  return script.slice(listStart, listEnd);
}

test("both verification entrypoints run an equivalent policy check set", () => {
  const policyScript = readRepoFile("scripts/check-policy.mjs");
  const powershell = readRepoFile("scripts/verify.ps1");
  const bash = readRepoFile("scripts/verify.sh");
  const checkList = readPowerShellCheckList(powershell);

  for (const { check, ps1 } of POLICY_CHECK_MATRIX) {
    // 用 includes 而不是正则：检查名里带空格，转义只会增加出错机会。
    assert.ok(
      policyScript.includes(`runCheck("${check}"`),
      `${check} 必须在 check-policy.mjs 注册`,
    );
    // 必须真的在 `$checks` 清单里，而不是只出现在注释里。
    assert.ok(checkList.includes(ps1), `${ps1} 必须在 verify.ps1 的 $checks 清单里`);
  }

  // 两个入口都要跑 CI 用的那个实现：这是「本地与 CI 判定一致」的保证。
  for (const [label, script] of [
    ["scripts/verify.sh", bash],
    ["scripts/verify.ps1", powershell],
  ]) {
    // 断言前先剔掉纯输出行（`Write-Host` / `echo`）。
    // 否则会误匹配到「回显这条命令」的那一行——那样即使把真正的调用删掉，
    // 断言照样绿（实测踩过）。两个入口的 node 命令写法也不同
    //（`node` vs `"${node_bin}"`），所以按「去掉回显后是否含调用」来判断。
    const invoked = stripOutputLines(script).includes(
      "scripts/check-policy.mjs --scope verify",
    );
    assert.ok(invoked, `${label} 必须真正调用 scripts/check-policy.mjs --scope verify`);
  }
});
