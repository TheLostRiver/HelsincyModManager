import assert from "node:assert/strict";
import {
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

const scriptsDir = dirname(fileURLToPath(import.meta.url));
const nodePolicyScript = join(scriptsDir, "check-policy.mjs");
const powershellFileSizeScript = join(scriptsDir, "check-file-size.ps1");

function writeFixtureFile(repoRoot, relativePath, content) {
  const fullPath = join(repoRoot, ...relativePath.split("/"));
  mkdirSync(dirname(fullPath), { recursive: true });
  writeFileSync(fullPath, content, "utf8");
}

function createPolicyFixture(
  t,
  {
    blockBytes = 64,
    maxLineLength = 16,
    maxLineLengthExcludePathPatterns = [],
    allowlist = [],
    files = {},
  } = {},
) {
  const repoRoot = mkdtempSync(join(tmpdir(), "hmm-policy-"));
  t.after(() => rmSync(repoRoot, { recursive: true, force: true }));

  const gitInit = spawnSync("git", ["init", "--quiet"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(gitInit.status, 0, gitInit.stderr);

  const policy = {
    requiredFiles: [],
    caseSensitiveFiles: [],
    requiredScripts: [],
    checkScopes: {
      verify: {
        excludePathPatterns: [],
      },
    },
    secretScan: {
      forceIncludePathPatterns: [],
    },
    fileSize: {
      blockBytes,
      maxLineLength,
      maxLineLengthExcludePathPatterns,
      block: {
        text: 100,
      },
      extensions: {
        text: [".txt", ".lock", ".yaml"],
      },
      allowlist: ["policy/project-policy.json", ...allowlist],
      excludePathPatterns: [],
    },
    forbiddenFiles: {
      extensions: [],
      pathPatterns: [],
    },
    secretPatterns: [],
    governanceFiles: [],
  };

  writeFixtureFile(
    repoRoot,
    "policy/project-policy.json",
    `${JSON.stringify(policy, null, 2)}\n`,
  );
  for (const [relativePath, content] of Object.entries(files)) {
    writeFixtureFile(repoRoot, relativePath, content);
  }

  return repoRoot;
}

function runNodePolicy(repoRoot) {
  return spawnSync(process.execPath, [nodePolicyScript, "--scope", "verify"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
}

function runPowerShellFileSize(repoRoot) {
  if (process.platform !== "win32") {
    return null;
  }

  return spawnSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      powershellFileSizeScript,
      "-Scope",
      "verify",
    ],
    {
      cwd: repoRoot,
      encoding: "utf8",
    },
  );
}

function resultOutput(result) {
  return `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
}

function assertPolicyResult(repoRoot, { succeeds, message }) {
  const nodeResult = runNodePolicy(repoRoot);
  assert.equal(nodeResult.status === 0, succeeds, resultOutput(nodeResult));
  if (message) {
    assert.match(resultOutput(nodeResult), message);
  }

  const powershellResult = runPowerShellFileSize(repoRoot);
  if (powershellResult) {
    assert.equal(
      powershellResult.status === 0,
      succeeds,
      resultOutput(powershellResult),
    );
    if (message) {
      assert.match(resultOutput(powershellResult), message);
    }
  }
}

test("file size checks reject a file above the byte limit", (t) => {
  const repoRoot = createPolicyFixture(t, {
    blockBytes: 32,
    maxLineLength: 128,
    files: {
      "src/oversized.txt": "a\n".repeat(20),
    },
  });

  assertPolicyResult(repoRoot, {
    succeeds: false,
    message: /src\/oversized\.txt exceeds hard byte limit: 40 \/ 32/,
  });
});

test("file size checks reject a single overlong line", (t) => {
  const repoRoot = createPolicyFixture(t, {
    blockBytes: 1024,
    maxLineLength: 8,
    files: {
      "src/long-line.txt": "123456789\n",
    },
  });

  assertPolicyResult(repoRoot, {
    succeeds: false,
    message: /src\/long-line\.txt exceeds hard line length: 9 at line 1 \/ 8/,
  });
});

test("file size checks accept a normal text file", (t) => {
  const repoRoot = createPolicyFixture(t, {
    files: {
      "src/normal.txt": "alpha\nbeta\n",
    },
  });

  assertPolicyResult(repoRoot, { succeeds: true });
});

test("file size checks honor the lockfile allowlist", (t) => {
  const repoRoot = createPolicyFixture(t, {
    blockBytes: 16,
    maxLineLength: 8,
    allowlist: ["Cargo.lock", "pnpm-lock.yaml"],
    files: {
      "Cargo.lock": "x".repeat(64),
      "pnpm-lock.yaml": "y".repeat(64),
    },
  });

  assertPolicyResult(repoRoot, { succeeds: true });
});
