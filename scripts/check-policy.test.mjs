import assert from "node:assert/strict";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
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
const powershellSecretScript = join(scriptsDir, "check-secrets.ps1");

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
    forceIncludePathPatterns = [],
    secretPatterns = [],
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
      preCommit: {
        excludePathPatterns: [".codex/**"],
      },
    },
    secretScan: {
      forceIncludePathPatterns,
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
    secretPatterns,
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

function runNodePolicy(repoRoot, scope = "verify") {
  return spawnSync(process.execPath, [nodePolicyScript, "--scope", scope], {
    cwd: repoRoot,
    encoding: "utf8",
  });
}

function runPowerShellScript(repoRoot, scriptPath, scope = "verify") {
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
      scriptPath,
      "-Scope",
      scope,
    ],
    {
      cwd: repoRoot,
      encoding: "utf8",
    },
  );
}

function runPowerShellFileSize(repoRoot) {
  return runPowerShellScript(repoRoot, powershellFileSizeScript);
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

function assertSecretResult(repoRoot, { succeeds, messages = [] }) {
  const nodeResult = runNodePolicy(repoRoot, "preCommit");
  assert.equal(nodeResult.status === 0, succeeds, resultOutput(nodeResult));
  for (const message of messages) {
    assert.match(resultOutput(nodeResult), message);
  }

  const powershellResult = runPowerShellScript(
    repoRoot,
    powershellSecretScript,
    "preCommit",
  );
  if (powershellResult) {
    assert.equal(
      powershellResult.status === 0,
      succeeds,
      resultOutput(powershellResult),
    );
    for (const message of messages) {
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

test("project policy assigns SQL files to a file-size category", () => {
  const policyPath = join(scriptsDir, "..", "policy", "project-policy.json");
  const policy = JSON.parse(readFileSync(policyPath, "utf8"));

  assert.deepEqual(policy.fileSize.extensions.sql, [".sql"]);
  assert.equal(policy.fileSize.block.sql, 1200);
});

test("secret checks scan forced Python and application SQL files", (t) => {
  const githubToken = `ghp_${"A".repeat(30)}`;
  const apiKey = `sk-${"B".repeat(20)}`;
  const repoRoot = createPolicyFixture(t, {
    blockBytes: 1024,
    maxLineLength: 256,
    forceIncludePathPatterns: [".codex/**"],
    secretPatterns: [
      {
        name: "GitHub classic token",
        regex: "ghp_[A-Za-z0-9_]{30,}",
      },
      {
        name: "OpenAI style API key",
        regex: "sk-[A-Za-z0-9]{20,}",
      },
    ],
    files: {
      ".codex/hooks/leak.py": `TOKEN = "${githubToken}"\n`,
      "src-tauri/migrations/leak.sql": `-- ${apiKey}\n`,
    },
  });

  assertSecretResult(repoRoot, {
    succeeds: false,
    messages: [
      /\.codex\/hooks\/leak\.py:1 matches secret pattern: GitHub classic token/,
      /src-tauri\/migrations\/leak\.sql:1 matches secret pattern: OpenAI style API key/,
    ],
  });
});

test("secret checks accept normal Python and SQL files", (t) => {
  const repoRoot = createPolicyFixture(t, {
    blockBytes: 1024,
    maxLineLength: 256,
    forceIncludePathPatterns: [".codex/**"],
    secretPatterns: [
      {
        name: "Fixture token",
        regex: "fixture_[A-Za-z0-9]{16}",
      },
    ],
    files: {
      ".codex/hooks/normal.py": "MODE = \"safe\"\n",
      "src-tauri/migrations/normal.sql": "SELECT 1;\n",
    },
  });

  assertSecretResult(repoRoot, { succeeds: true });
});
