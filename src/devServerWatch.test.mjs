import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { createServer } from "vite";
import viteConfig, { createDevWatchIgnored } from "../vite.config.ts";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const fixtureRoot = join(tmpdir(), "hmm-watch-policy-unit");
const ignoredDirectories = [
  "src-tauri", "target", ".worktrees", ".claude", ".codex", ".agents",
  ".planning", ".tmp", "tmp", ".vite", "armor-data",
];

for (const directory of ignoredDirectories) {
  test(`dev watcher prunes root ${directory} before descending`, () => {
    const ignored = createDevWatchIgnored(fixtureRoot);
    assert.equal(ignored(join(fixtureRoot, directory)), true, "Directory itself must be pruned");
    assert.equal(ignored(join(fixtureRoot, directory, "nested", "entry.ts")), true);
  });
}

for (const relativePath of [
  "src/entry.ts", "src/target/entry.ts", "public/target/asset.txt", "src/tmp/component.tsx",
  "src/.worktrees/component.tsx", ".env", ".env.local", "vite.config.ts", "package.json",
  "new-frontend-directory/entry.ts", ".unknown-directory/entry.ts",
]) {
  test(`dev watcher preserves ${relativePath}`, () => {
    assert.equal(createDevWatchIgnored(fixtureRoot)(join(fixtureRoot, relativePath)), false);
  });
}

test("dev watcher leaves the repository root and outside dependencies observable", () => {
  const ignored = createDevWatchIgnored(fixtureRoot);
  assert.equal(ignored(fixtureRoot), false);
  assert.equal(ignored(join(fixtureRoot, "..", "sibling", "target", "entry.ts")), false);
});

test("Vite configuration applies root pruning without excluding frontend paths", () => {
  const ignored = viteConfig.server.watch.ignored;
  for (const directory of ignoredDirectories) {
    assert.equal(ignored(join(repositoryRoot, directory, "entry.ts")), true, directory);
  }
  assert.equal(ignored(join(repositoryRoot, "src", "target", "entry.ts")), false);
  assert.equal(ignored(join(repositoryRoot, ".env")), false);
});

test("Vite watcher prunes non-frontend trees and still reloads changed source", { timeout: 20000 }, async (t) => {
  const temporaryRoot = resolve(tmpdir());
  const root = await mkdtemp(join(temporaryRoot, "hmm-vite-watch-"));
  let server;
  t.after(async () => {
    try {
      await server?.close();
    } finally {
      assert.equal(dirname(root), temporaryRoot, "Cleanup must remain inside the owned temp root");
      await rm(root, { recursive: true, force: true });
    }
  });

  for (const directory of [...ignoredDirectories, "src/target", "public/target"]) {
    const folder = join(root, directory, "nested");
    await mkdir(folder, { recursive: true });
    await writeFile(join(folder, "entry.ts"), "export const value = 1;\n");
  }
  await writeFile(join(root, "package.json"), JSON.stringify({ type: "module" }));
  await writeFile(join(root, ".env"), "HMM_WATCH_FIXTURE=1\n");
  const sourceFile = join(root, "src", "target", "nested", "entry.ts");
  const ignoredChecks = new Map();
  const ignored = createDevWatchIgnored(root);
  server = await createServer({
    configFile: false,
    root,
    logLevel: "silent",
    optimizeDeps: { noDiscovery: true },
    server: { host: "127.0.0.1", port: 0, watch: { ignored: (entry) => {
      const result = ignored(entry);
      ignoredChecks.set(resolve(entry), result);
      return result;
    } } },
  });
  // Vite adds several roots; the first ready event can precede recursive registration.
  const deadline = Date.now() + 10000;
  let watchedDirectories;
  while (true) {
    watchedDirectories = new Set(Object.keys(server.watcher.getWatched()).map((entry) => resolve(entry)));
    if (ignoredDirectories.every((directory) => ignoredChecks.has(join(root, directory)))
      && watchedDirectories.has(dirname(sourceFile))
      && watchedDirectories.has(join(root, "public", "target", "nested"))) break;
    if (Date.now() >= deadline) throw new Error("Vite watcher initialization timed out");
    await new Promise((fulfill) => setTimeout(fulfill, 25));
  }
  for (const directory of ignoredDirectories) {
    assert.equal(ignoredChecks.get(join(root, directory)), true, directory);
    assert.equal(watchedDirectories.has(join(root, directory)), false, directory);
    assert.equal(watchedDirectories.has(join(root, directory, "nested")), false, directory);
  }
  assert.ok(server.watcher.getWatched()[root].includes(".env"));

  await server.listen();
  const initial = await server.environments.client.transformRequest("/src/target/nested/entry.ts");
  assert.match(initial.code, /value = 1/);
  const sourceChanged = new Promise((fulfill, reject) => {
    const timer = setTimeout(() => {
      server.watcher.off("change", changed);
      reject(new Error("Expected source change was not observed"));
    }, 5000);
    function changed(changedPath) {
      if (resolve(changedPath) !== sourceFile) return;
      clearTimeout(timer);
      server.watcher.off("change", changed);
      fulfill();
    }
    server.watcher.on("change", changed);
  });
  await writeFile(sourceFile, "export const value = 2;\n");
  await sourceChanged;
  const updated = await server.environments.client.transformRequest("/src/target/nested/entry.ts");
  assert.match(updated.code, /value = 2/, "File changes must invalidate Vite's transformed module");
});
