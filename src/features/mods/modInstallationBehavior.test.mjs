import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "features/game-setup/GameSetupProvider.tsx": "export const useGameSetup = () => ({ status: globalThis.__installationTest.game });",
  "features/mods/modInstallationApi.ts": "export const getModInstallationContext = (gameId) => globalThis.__installationTest.request(gameId);",
  "features/profiles/profileApi.ts": `
    export const getActiveProfile = async () => ({ id: globalThis.__installationTest.account });
    export const setActiveProfile = async (id) => { globalThis.__installationTest.account = id; };`,
});

const { ModInstallationProvider, useModInstallation } = await import("./ModInstallationProvider.tsx");
const { ActiveProfileProvider, useActiveProfile } = await import("../profiles/ActiveProfileProvider.tsx");
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const options = { concurrency: false, timeout: 5000 };
const context = (scopeId) => ({ gameId: "mhw", installationId: `installation-${scopeId}`, scopeId });

async function mount(t, configured = true) {
  let root, installation, account;
  const api = {
    game: { kind: configured ? "configured" : "not_configured", gameId: "mhw", rootDir: "fixture-a" },
    account: "save-a", requests: [], renders: [],
    request(gameId) { return new Promise((resolve, reject) => api.requests.push({ gameId, resolve, reject })); },
  };
  globalThis.__installationTest = api;
  function Capture() {
    installation = useModInstallation();
    account = useActiveProfile();
    api.renders.push({ scope: installation.installationScopeId, game: api.game, account: account.activeProfileId });
    return null;
  }
  const tree = () => React.createElement(React.StrictMode, null,
    React.createElement(ModInstallationProvider, null,
      React.createElement(ActiveProfileProvider, null, React.createElement(Capture))));
  await act(async () => { root = TestRenderer.create(tree()); });
  t.after(async () => { await act(async () => root.unmount()); delete globalThis.__installationTest; });
  return {
    api, get installation() { return installation; }, get account() { return account; },
    resolve: async (index, value) => { await act(async () => api.requests[index].resolve(value)); },
    reject: async (index, error) => { await act(async () => api.requests[index].reject(error)); },
    game: async (game) => { api.game = game; await act(async () => root.update(tree())); },
  };
}

test("changing and deleting save profiles keeps the same Mod installation context", options, async (t) => {
  const h = await mount(t);
  assert.equal(h.api.requests.length, 1, "StrictMode must not register the installation twice");
  await h.resolve(0, context("mods-a"));
  const initial = h.installation.installationScope;
  await act(async () => h.account.setActiveProfile("save-b"));
  assert.equal(h.account.activeProfileId, "save-b");
  assert.equal(h.installation.installationScope, initial);
  // The server selects another save profile after deletion; Mod identity remains independent.
  h.api.account = "save-c";
  await act(async () => h.account.refreshActiveProfile());
  assert.equal(h.account.activeProfileId, "save-c");
  assert.equal(h.installation.installationScopeId, "mods-a");
  assert.equal(h.api.requests.length, 1);
});

test("changing game directories masks old context immediately and ignores a late response", options, async (t) => {
  const h = await mount(t);
  const secondGame = { kind: "configured", gameId: "mhw", rootDir: "fixture-b" };
  await h.game(secondGame);
  assert.equal(h.installation.installationScopeId, null);
  assert.equal(h.api.requests.length, 2);
  await h.resolve(1, context("mods-b"));
  await h.resolve(0, context("mods-a"));
  assert.equal(h.installation.installationScopeId, "mods-b");
  assert.ok(h.api.renders.filter((render) => render.game === secondGame).every((render) => render.scope !== "mods-a"));
  const thirdGame = { kind: "configured", gameId: "mhw", rootDir: "fixture-c" };
  await h.game(thirdGame);
  assert.equal(h.installation.installationScopeId, null);
  assert.ok(h.api.renders.filter((render) => render.game === thirdGame).every((render) => render.scope === null));
});

test("an unavailable directory clears the context and makes no registration request", options, async (t) => {
  const h = await mount(t, false);
  assert.equal(h.api.requests.length, 0);
  assert.equal(h.installation.installationScope.code, "mod_installation_game_unavailable");
  await h.game({ kind: "configured", gameId: "mhw", rootDir: "fixture-a" });
  await h.resolve(0, context("mods-a"));
  await h.game({ kind: "invalid", gameId: "mhw" });
  assert.equal(h.installation.installationScopeId, null);
  assert.equal(h.api.requests.length, 1);
});

test("ambiguous legacy state is visible and retry does not invent a default scope", options, async (t) => {
  const h = await mount(t);
  await h.reject(0, { code: "mod_installation_legacy_ambiguous" });
  assert.equal(h.installation.installationScopeId, null);
  assert.equal(h.installation.installationScope.code, "mod_installation_legacy_ambiguous");
  await act(async () => h.installation.refreshInstallationScope());
  assert.equal(h.installation.installationScope.status, "loading");
  await h.resolve(1, context("retained-namespace"));
  assert.equal(h.installation.installationScopeId, "retained-namespace");
});

test("a context for another game is rejected", options, async (t) => {
  const h = await mount(t);
  await h.resolve(0, { ...context("mods-a"), gameId: "other-game" });
  assert.equal(h.installation.installationScopeId, null);
  assert.equal(h.installation.installationScope.status, "unavailable");
});
