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

function policyGlobToRegex(pattern) {
  const normalized = pattern.replaceAll("\\", "/");
  const escaped = normalized.replace(/[|\\{}()[\]^$+?.]/g, "\\$&");
  const regex = escaped.replaceAll("**", "\0").replaceAll("*", "[^/]*").replaceAll("\0", ".*");
  return new RegExp(`^${regex}$`);
}

function readCodeownerPrefixes(content) {
  return content
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line && !line.startsWith("#"))
    .map((line) => line.split(/\s+/, 1)[0].replace(/^\/+/, ""));
}

function governancePatternToPrefix(pattern) {
  const normalized = pattern.replaceAll("\\", "/");
  if (normalized.endsWith("/**")) {
    return normalized.slice(0, -2);
  }

  assert.doesNotMatch(normalized, /[*?[\]]/, `Unsupported governance glob: ${pattern}`);
  return normalized;
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

test("line-length exclusions exempt matching paths from only the line-length limit", (t) => {
  const repoRoot = createPolicyFixture(t, {
    blockBytes: 1024,
    maxLineLength: 8,
    maxLineLengthExcludePathPatterns: ["docs/**"],
    files: {
      "docs/long-line.txt": "123456789\n",
    },
  });

  assertPolicyResult(repoRoot, { succeeds: true });
});

test("line-length exclusions still enforce the byte limit", (t) => {
  const repoRoot = createPolicyFixture(t, {
    blockBytes: 8,
    maxLineLength: 8,
    maxLineLengthExcludePathPatterns: ["docs/**"],
    files: {
      "docs/oversized.txt": "123456789\n",
    },
  });

  assertPolicyResult(repoRoot, {
    succeeds: false,
    message: /docs\/oversized\.txt exceeds hard byte limit: 10 \/ 8/,
  });
});

test("line-length exclusions still enforce the line-count limit", (t) => {
  const repoRoot = createPolicyFixture(t, {
    blockBytes: 1024,
    maxLineLength: 8,
    maxLineLengthExcludePathPatterns: ["docs/**"],
    files: {
      "docs/too-many-lines.txt": "x\n".repeat(101),
    },
  });

  assertPolicyResult(repoRoot, {
    succeeds: false,
    message: /docs\/too-many-lines\.txt exceeds hard line limit: 101 \/ 100/,
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

test("CODEOWNERS governance prefixes stay aligned with policy and docs", () => {
  const repoRoot = join(scriptsDir, "..");
  const policy = JSON.parse(
    readFileSync(join(repoRoot, "policy", "project-policy.json"), "utf8"),
  );
  const codeownersPrefixes = readCodeownerPrefixes(
    readFileSync(join(repoRoot, ".github", "CODEOWNERS"), "utf8"),
  );
  const policyPrefixes = policy.governanceFiles.map(governancePatternToPrefix);
  const governanceRegexes = policy.governanceFiles.map(policyGlobToRegex);

  assert.deepEqual(
    [...policyPrefixes].sort(),
    [...codeownersPrefixes].sort(),
    "policy.governanceFiles must describe the same prefixes as CODEOWNERS",
  );

  const uncovered = [];
  for (const prefix of codeownersPrefixes) {
    const probes = prefix.endsWith("/")
      ? [`${prefix}__governance_probe__`, `${prefix}nested/__governance_probe__`]
      : [prefix];
    for (const probe of probes) {
      if (!governanceRegexes.some((regex) => regex.test(probe))) {
        uncovered.push(probe);
      }
    }
  }
  assert.deepEqual(uncovered, [], "every CODEOWNERS prefix must be covered by a governance glob");

  const governanceDoc = readFileSync(join(repoRoot, "docs", "GOVERNANCE.md"), "utf8");
  const governanceList = governanceDoc.match(
    /治理文件包括：\r?\n(?<body>[\s\S]*?)\r?\nCODEOWNERS 本身/,
  );
  assert.ok(governanceList?.groups?.body, "docs/GOVERNANCE.md must list governance files");
  const documentedPrefixes = [...governanceList.groups.body.matchAll(/^- `([^`]+)`/gm)].map(
    (match) => match[1],
  );
  assert.deepEqual(
    [...documentedPrefixes].sort(),
    [...codeownersPrefixes].sort(),
    "docs/GOVERNANCE.md must describe the same prefixes as CODEOWNERS",
  );
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
