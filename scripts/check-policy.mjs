#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { execFileSync, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const defaultRepoRoot = path.resolve(scriptDir, "..");

function parseArgs(argv) {
  const options = { scope: "verify" };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--scope") {
      options.scope = argv[index + 1];
      index += 1;
      continue;
    }

    if (arg === "-h" || arg === "--help") {
      console.log("Usage: node scripts/check-policy.mjs [--scope verify|preCommit]");
      process.exit(0);
    }

    throw new Error(`Unknown argument: ${arg}`);
  }

  return options;
}

function getRepoRoot() {
  try {
    return execFileSync("git", ["rev-parse", "--show-toplevel"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
  } catch {
    return defaultRepoRoot;
  }
}

function normalizeRepoRelativePath(relativePath) {
  const segments = [];
  for (const part of String(relativePath).split(/[\\/]+/)) {
    if (!part || part === ".") {
      continue;
    }

    if (part === "..") {
      if (segments.length === 0) {
        throw new Error(`Path escapes repository root: ${relativePath}`);
      }
      segments.pop();
      continue;
    }

    segments.push(part);
  }

  return segments.join("/");
}

function toRepoPath(relativePath) {
  return normalizeRepoRelativePath(relativePath).replaceAll("\\", "/");
}

function joinRepoPath(repoRoot, relativePath) {
  return path.join(repoRoot, ...toRepoPath(relativePath).split("/"));
}

function readPolicy(repoRoot) {
  return JSON.parse(fs.readFileSync(joinRepoPath(repoRoot, "policy/project-policy.json"), "utf8"));
}

function getGitCandidateFiles(repoRoot) {
  const output = execFileSync(
    "git",
    ["-c", "core.quotePath=false", "ls-files", "--cached", "--others", "--exclude-standard"],
    { cwd: repoRoot, encoding: "utf8" },
  );
  return output
    .split(/\r?\n/)
    .map((item) => item.trim())
    .filter(Boolean)
    .map((item) => item.replaceAll("\\", "/"));
}

function testExactPathCase(repoRoot, relativePath) {
  let current = repoRoot;
  for (const part of toRepoPath(relativePath).split("/")) {
    if (!fs.existsSync(current)) {
      return false;
    }

    const match = fs.readdirSync(current, { withFileTypes: true }).find((entry) => entry.name === part);
    if (!match) {
      return false;
    }

    current = path.join(current, match.name);
  }

  return true;
}

function globToRegex(pattern) {
  const normalized = pattern.replaceAll("\\", "/");
  const escaped = normalized.replace(/[|\\{}()[\]^$+?.]/g, "\\$&");
  const regex = escaped.replaceAll("**", "\0").replaceAll("*", "[^/]*").replaceAll("\0", ".*");
  return new RegExp(`^${regex}$`);
}

function getScopeExcludePatterns(policy, scope) {
  return policy.checkScopes?.[scope]?.excludePathPatterns ?? [];
}

function pathMatchesAny(relativePath, regexes) {
  return regexes.some((regex) => regex.test(relativePath.replaceAll("\\", "/")));
}

function selectIncludedFiles(files, excludePatterns) {
  const excludeRegexes = excludePatterns.map(globToRegex);
  return files.filter((file) => !pathMatchesAny(file, excludeRegexes));
}

function selectMatchingFiles(files, includePatterns) {
  const includeRegexes = includePatterns.map(globToRegex);
  return files.filter((file) => pathMatchesAny(file, includeRegexes));
}

function mergeFileLists(primaryFiles, additionalFiles) {
  const seen = new Set();
  const merged = [];
  for (const file of [...primaryFiles, ...additionalFiles]) {
    const normalized = file.replaceAll("\\", "/");
    if (!seen.has(normalized)) {
      seen.add(normalized);
      merged.push(normalized);
    }
  }
  return merged;
}

function writeErrors(title, errors) {
  if (errors.length === 0) {
    return;
  }

  console.error(title);
  for (const error of errors) {
    console.error(`  - ${error}`);
  }
}

function runCheck(name, check) {
  const errors = check();
  if (errors.length > 0) {
    writeErrors(`${name} failed:`, errors);
    process.exit(1);
  }

  console.log(`${name} passed.`);
}

function checkPolicy(repoRoot, policy) {
  const errors = [];
  for (const file of policy.requiredFiles ?? []) {
    if (!testExactPathCase(repoRoot, file)) {
      errors.push(`Required file is missing or has wrong case: ${file}`);
    }
  }

  for (const file of policy.caseSensitiveFiles ?? []) {
    if (!testExactPathCase(repoRoot, file)) {
      errors.push(`Case-sensitive file mismatch: ${file}`);
    }
  }

  for (const script of policy.requiredScripts ?? []) {
    if (!testExactPathCase(repoRoot, script)) {
      errors.push(`Required script is missing or has wrong case: ${script}`);
    }
  }

  for (const entry of fs.readdirSync(repoRoot, { withFileTypes: true })) {
    if (entry.name.toLowerCase() === "agents.md" && entry.name !== "AGENTS.md") {
      errors.push(`Agent guide must be named AGENTS.md; found: ${entry.name}`);
    }
  }

  return errors;
}

function checkWhitespace(repoRoot, policy, scope) {
  const pathspecs = ["--", "."];
  for (const pattern of getScopeExcludePatterns(policy, scope)) {
    pathspecs.push(`:(exclude)${pattern}`);
  }

  const result = spawnSync("git", ["diff", "--check", ...pathspecs], {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: "inherit",
  });

  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }

  return [];
}

function getCategory(policy, relativePath) {
  const extension = path.extname(relativePath).toLowerCase();
  for (const [category, extensions] of Object.entries(policy.fileSize?.extensions ?? {})) {
    if (extensions.map((item) => item.toLowerCase()).includes(extension)) {
      return category;
    }
  }

  return null;
}

function checkFileSize(repoRoot, policy, files, scope) {
  const errors = [];
  const allowlist = new Set(policy.fileSize?.allowlist ?? []);
  const byteLimit = policy.fileSize?.blockBytes ?? null;
  const maxLineLength = policy.fileSize?.maxLineLength ?? null;
  const maxLineLengthExcludeRegexes = (
    policy.fileSize?.maxLineLengthExcludePathPatterns ?? []
  ).map(globToRegex);
  const excludePatterns = [
    ...(policy.fileSize?.excludePathPatterns ?? []),
    ...getScopeExcludePatterns(policy, scope),
  ];
  const excludeRegexes = excludePatterns.map(globToRegex);

  for (const file of files) {
    const normalized = file.replaceAll("\\", "/");
    if (pathMatchesAny(normalized, excludeRegexes) || allowlist.has(normalized)) {
      continue;
    }

    const fullPath = joinRepoPath(repoRoot, normalized);
    if (!fs.existsSync(fullPath) || !fs.statSync(fullPath).isFile()) {
      continue;
    }

    const fileSize = fs.statSync(fullPath).size;
    if (byteLimit !== null && fileSize > byteLimit) {
      errors.push(`${normalized} exceeds hard byte limit: ${fileSize} / ${byteLimit}`);
    }

    const category = getCategory(policy, normalized);
    if (!category) {
      continue;
    }

    const limit = policy.fileSize?.block?.[category] ?? null;
    const checkMaxLineLength =
      maxLineLength !== null && !pathMatchesAny(normalized, maxLineLengthExcludeRegexes);
    if (limit === null && !checkMaxLineLength) {
      continue;
    }

    const content = fs.readFileSync(fullPath, "utf8");
    const lines = content.length === 0 ? [] : content.split(/\r\n|\n|\r/);
    if (lines.at(-1) === "") {
      lines.pop();
    }

    const lineCount = lines.length;
    if (limit !== null && lineCount > limit) {
      errors.push(`${normalized} exceeds hard line limit: ${lineCount} / ${limit}`);
    }

    if (checkMaxLineLength) {
      let longestLineLength = 0;
      let longestLineNumber = 0;
      for (let index = 0; index < lines.length; index += 1) {
        if (lines[index].length > longestLineLength) {
          longestLineLength = lines[index].length;
          longestLineNumber = index + 1;
        }
      }

      if (longestLineLength > maxLineLength) {
        errors.push(
          `${normalized} exceeds hard line length: ${longestLineLength} at line ${longestLineNumber} / ${maxLineLength}`,
        );
      }
    }
  }

  return errors;
}

function checkForbiddenFiles(policy, files, scope) {
  const errors = [];
  const includedFiles = selectIncludedFiles(files, getScopeExcludePatterns(policy, scope));
  const forbiddenExtensions = new Set((policy.forbiddenFiles?.extensions ?? []).map((item) => item.toLowerCase()));
  const pathRegexes = (policy.forbiddenFiles?.pathPatterns ?? []).map(globToRegex);

  for (const file of includedFiles) {
    const normalized = file.replaceAll("\\", "/");
    const extension = path.extname(normalized).toLowerCase();
    if (forbiddenExtensions.has(extension)) {
      errors.push(`Forbidden file type: ${normalized}`);
      continue;
    }

    if (pathMatchesAny(normalized, pathRegexes)) {
      errors.push(`Forbidden path: ${normalized}`);
    }
  }

  return errors;
}

function checkMarkdownLinks(repoRoot, policy, files, scope) {
  const errors = [];
  const includedFiles = selectIncludedFiles(files, getScopeExcludePatterns(policy, scope));
  const markdownFiles = includedFiles.filter((file) => path.extname(file).toLowerCase() === ".md");
  const linkPattern = /\[[^\]]+\]\(([^)]+)\)/g;

  for (const file of markdownFiles) {
    const fullPath = joinRepoPath(repoRoot, file);
    if (!fs.existsSync(fullPath) || !fs.statSync(fullPath).isFile()) {
      continue;
    }

    const content = fs.readFileSync(fullPath, "utf8");
    const baseDir = path.posix.dirname(file.replaceAll("\\", "/"));
    for (const match of content.matchAll(linkPattern)) {
      let target = match[1].trim();
      if (!target) {
        continue;
      }

      target = target.replace(/^<|>$/g, "");
      if (target.startsWith("#") || /^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(target)) {
        continue;
      }

      const targetWithoutAnchor = target.split("#", 2)[0];
      if (!targetWithoutAnchor) {
        continue;
      }

      const decodedTarget = decodeURIComponent(targetWithoutAnchor);
      const candidate = baseDir === "." ? decodedTarget : `${baseDir}/${decodedTarget}`;

      let normalizedCandidate;
      try {
        normalizedCandidate = normalizeRepoRelativePath(candidate);
      } catch {
        errors.push(`${file} link escapes repository root: ${target}`);
        continue;
      }

      if (!testExactPathCase(repoRoot, normalizedCandidate)) {
        errors.push(`${file} contains invalid link: ${target}`);
      }
    }
  }

  return errors;
}

function walkFiles(root) {
  if (!fs.existsSync(root) || !fs.statSync(root).isDirectory()) {
    return [];
  }

  const files = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const fullPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...walkFiles(fullPath));
    } else if (entry.isFile()) {
      files.push(fullPath);
    }
  }
  return files;
}

function getRepoRelativePath(repoRoot, fullPath) {
  return path.relative(repoRoot, fullPath).replaceAll(path.sep, "/");
}

function checkFrontendBoundaries(repoRoot) {
  const errors = [];
  const packageJsonPath = joinRepoPath(repoRoot, "package.json");
  const srcRoot = joinRepoPath(repoRoot, "src");
  if (!fs.existsSync(packageJsonPath) || !fs.existsSync(srcRoot)) {
    console.log("Frontend boundary check skipped: frontend scaffold not found.");
    return errors;
  }

  const dashboardRoot = joinRepoPath(repoRoot, "src/features/dashboard");
  for (const file of walkFiles(dashboardRoot)) {
    const extension = path.extname(file).toLowerCase();
    const relative = getRepoRelativePath(repoRoot, file);
    if (extension === ".ts" || extension === ".tsx") {
      const content = fs.readFileSync(file, "utf8");
      if (content.includes("sidebarMode") || content.includes("useSidebarMode")) {
        errors.push(`Dashboard file must not read sidebar mode: ${relative}`);
      }
    }

    if (extension === ".css") {
      const content = fs.readFileSync(file, "utf8");
      if (/\[data-sidebar-mode/.test(content)) {
        errors.push(`Dashboard CSS must not branch by sidebar mode: ${relative}`);
      }
    }
  }

  for (const relativePath of [
    "src/features/dashboard/FloatingDashboardPage.tsx",
    "src/features/dashboard/ClassicDashboardPage.tsx",
  ]) {
    if (fs.existsSync(joinRepoPath(repoRoot, relativePath))) {
      errors.push(`Do not duplicate dashboard page by sidebar mode: ${relativePath}`);
    }
  }

  const navDefinitionFiles = walkFiles(srcRoot).filter((file) => {
    const name = path.basename(file);
    return name.endsWith("navItems.ts") || name.endsWith("NavItems.ts");
  });

  if (navDefinitionFiles.length !== 1) {
    const relativeFiles = navDefinitionFiles.map((file) => getRepoRelativePath(repoRoot, file));
    let message = `Expected exactly one navItems file under src, found ${navDefinitionFiles.length}.`;
    if (relativeFiles.length > 0) {
      message += ` Found: ${relativeFiles.join(", ")}`;
    }
    errors.push(message);
  }

  return errors;
}

function checkSecrets(repoRoot, policy, files, scope) {
  const scopeFiles = selectIncludedFiles(files, getScopeExcludePatterns(policy, scope));
  const forceIncludedFiles = selectMatchingFiles(files, policy.secretScan?.forceIncludePathPatterns ?? []);
  const filesToScan = mergeFileLists(scopeFiles, forceIncludedFiles);
  const errors = [];
  const textExtensions = new Set([
    ".md",
    ".txt",
    ".json",
    ".toml",
    ".yml",
    ".yaml",
    ".ps1",
    ".psm1",
    ".py",
    ".sql",
    ".rs",
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    ".cjs",
    ".css",
    ".html",
    ".mjs",
    ".sh",
    ".bash",
    ".zsh",
  ]);
  const secretPatterns = (policy.secretPatterns ?? []).map((pattern) => ({
    name: pattern.name,
    regex: new RegExp(pattern.regex),
  }));

  for (const file of filesToScan) {
    const normalized = file.replaceAll("\\", "/");
    if (!textExtensions.has(path.extname(normalized).toLowerCase())) {
      continue;
    }

    const fullPath = joinRepoPath(repoRoot, normalized);
    if (!fs.existsSync(fullPath) || !fs.statSync(fullPath).isFile()) {
      continue;
    }

    const lines = fs.readFileSync(fullPath, "utf8").split(/\r\n|\n|\r/);
    for (let index = 0; index < lines.length; index += 1) {
      for (const pattern of secretPatterns) {
        if (pattern.regex.test(lines[index])) {
          errors.push(`${normalized}:${index + 1} matches secret pattern: ${pattern.name}`);
        }
      }
    }
  }

  return errors;
}

const { scope } = parseArgs(process.argv.slice(2));
const repoRoot = getRepoRoot();
const policy = readPolicy(repoRoot);
const files = getGitCandidateFiles(repoRoot);

runCheck("Policy check", () => checkPolicy(repoRoot, policy));
runCheck("Whitespace check", () => checkWhitespace(repoRoot, policy, scope));
runCheck("File size check", () => checkFileSize(repoRoot, policy, files, scope));
runCheck("Forbidden files check", () => checkForbiddenFiles(policy, files, scope));
runCheck("Markdown link check", () => checkMarkdownLinks(repoRoot, policy, files, scope));
runCheck("Frontend boundary check", () => checkFrontendBoundaries(repoRoot));
runCheck("Secret scan", () => checkSecrets(repoRoot, policy, files, scope));
