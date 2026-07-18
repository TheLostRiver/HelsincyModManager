import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { registerHooks } from "node:module";
import { test } from "node:test";

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (
      specifier === "./modLibraryLoadState"
      && context.parentURL?.endsWith("/modLibraryRecoveryRefresh.ts")
    ) {
      return nextResolve("./modLibraryLoadState.ts", context);
    }
    return nextResolve(specifier, context);
  },
});

const { refreshModLibraryDurableStatuses } = await import("./modLibraryRecoveryRefresh.ts");

const source = readFileSync("src/features/mods/modLibraryRecoveryRefresh.ts", "utf8");

test("durable status refresh limits both status calls to unique supplied item ids", () => {
  assert.match(source, /new Set\(items\.map\(\(item\) => item\.id\)\)/);
  assert.match(source, /loadManifestStatuses\(modIds\)/);
  assert.match(source, /loadRecoveryStatuses\(modIds\)/);
  assert.doesNotMatch(source, /getModLibrary|queryModLibrary|list_analysis|libraryTotal/);
});

test("durable status refresh overlays manifest then recovery facts", () => {
  const manifestApplyIndex = source.indexOf("applyInstallManifestStatusSummaries(items, manifestStatuses)");
  const recoveryApplyIndex = source.indexOf("applyInstallRecoverySummaries(itemsWithManifestStatus, recoveryStatuses)");

  assert.ok(manifestApplyIndex >= 0);
  assert.ok(recoveryApplyIndex > manifestApplyIndex);
  assert.match(source, /items:\s*applyInstallManifestUnavailable\(items\)/);
  assert.match(source, /items:\s*applyInstallRecoveryUnavailable\(itemsWithManifestStatus\)/);
});

test("durable status refresh treats empty pages as verified without calling loaders", () => {
  const emptyGuardIndex = source.indexOf("if (modIds.length === 0)");
  const manifestCallIndex = source.indexOf("loadManifestStatuses(modIds)");

  assert.ok(emptyGuardIndex >= 0);
  assert.ok(manifestCallIndex > emptyGuardIndex);
  assert.match(source.slice(emptyGuardIndex, manifestCallIndex), /return \{ items, verified: true \}/);
});

test("terminal status probes contain no filesystem or package content fields", () => {
  const probe = source.match(/export function createModLibraryStatusProbe[\s\S]*?\n\}/)?.[0];

  assert.ok(probe);
  assert.match(probe, /status:\s*"unknown"/);
  assert.match(probe, /categoryLabels:\s*\[\]/);
  assert.doesNotMatch(probe, /path|archive|sandbox|cache|manifest|backup|content/i);
});

test("manifest loader failure fails closed for every page item before recovery scan", async () => {
  const calls = [];
  const result = await refreshModLibraryDurableStatuses(
    [
      {
        id: "not-installed-mod",
        name: "Not installed Mod",
        sizeLabel: "1 KB",
        status: "not_installed",
        categoryLabels: [],
      },
      {
        id: "legacy-conflict-mod",
        name: "Legacy Conflict Mod",
        sizeLabel: "2 KB",
        status: "conflict",
        categoryLabels: [],
      },
    ],
    {
      loadManifestStatuses: async (modIds) => {
        calls.push(["manifest", modIds]);
        throw new Error("manifest unavailable");
      },
      loadRecoveryStatuses: async (modIds) => {
        calls.push(["recovery", modIds]);
        return [];
      },
    },
  );

  assert.equal(result.verified, false);
  assert.deepEqual(result.items.map((item) => item.status), ["unknown", "unknown"]);
  assert.deepEqual(result.items.map((item) => item.installSummary?.status), ["unknown", "unknown"]);
  assert.deepEqual(calls.map(([kind]) => kind), ["manifest"]);
});

test("recovery detail failure retains the existing not-installed fallback semantics", async () => {
  const result = await refreshModLibraryDurableStatuses(
    [
      {
        id: "not-installed-mod",
        name: "Not installed Mod",
        sizeLabel: "1 KB",
        status: "not_installed",
        categoryLabels: [],
      },
    ],
    {
      loadManifestStatuses: async () => [
        {
          profileId: "default",
          modId: "not-installed-mod",
          status: "not_installed",
          managedFileCount: 0,
          backupCount: 0,
        },
      ],
      loadRecoveryStatuses: async () => {
        throw new Error("recovery unavailable");
      },
    },
  );

  assert.equal(result.verified, false);
  assert.equal(result.items[0].status, "not_installed");
  assert.equal(result.items[0].installSummary?.status, "not_installed");
});
