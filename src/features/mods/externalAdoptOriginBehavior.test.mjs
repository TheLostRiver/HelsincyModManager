import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "features/mods/modLibraryApi.ts": "export const getModDetail = (input) => globalThis.__adoptOrigin.detail(input);",
  "features/mods/modCategoryApi.ts": "export const listCategories = async () => []; export const getModCategories = async () => []; export const setModCategories = async () => {};",
  "features/mods/modMetadataApi.ts": "export const updateModMetadata = async () => {};",
  "features/mods/useExternalModState.ts": "export const useExternalModState = () => globalThis.__adoptOrigin.workflow;",
  "features/replacements/ReplacementTargetPanel.tsx": "export const ReplacementTargetPanel = () => null;",
  "shared/feedback/useModalFocusTrap.ts": "export const useModalFocusTrap = () => {};",
  "shared/feedback/index.ts": `import React from "react"; export const Dialog = ({ open, children, footer }) => open ? React.createElement("section", { role: "alertdialog" }, children, footer) : null;`,
}, { "react-dom": "export const createPortal = (children) => children;" });
const { ModDetailDialog } = await import("./ModDetailDialog.tsx");
const { I18nProvider } = await import("../../shared/i18n/I18nProvider.tsx");
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const options = { concurrency: false, timeout: 5000 };
const migrated = { kind: "external_import", adapterId: "hunting_box_directory_v1", importedAtUnixMillis: null };
const detail = (origin, id = "a") => ({ id, name: "Fixture Mod", packageId: "pkg", metadata: { tags: [], dependencies: [] }, origin });

async function mount(t) {
  const requests = [];
  let root, adoptions = 0;
  const oldWindow = globalThis.window, oldDocument = globalThis.document;
  globalThis.window = { localStorage: { getItem: () => null, setItem() {} }, addEventListener() {}, removeEventListener() {}, clearTimeout() {} };
  globalThis.document = { body: {}, documentElement: {} };
  globalThis.__adoptOrigin = {
    detail: (input) => new Promise((resolve, reject) => requests.push({ input, resolve, reject })),
    workflow: { state: { summary: { state: "installed", matchedFileCount: 1, missingFileCount: 0, changedFileCount: 0, unreadableFileCount: 0, occupiedBy: [], files: [{ targetPath: "nativePC/fixture.mod3", state: "matched" }] }, stale: false, lastError: null },
      loaded: true, listenerReady: true, scanning: false, adopting: false, scanErrorCode: null, adoptErrorCode: null,
      startScan() {}, startAdopt() { adoptions += 1; }, refresh() {} },
  };
  const tree = (modId = "a") => React.createElement(I18nProvider, null, React.createElement(ModDetailDialog, {
    modId, initialTab: "details", gameId: "mhw", profileId: "profile", installStatus: "not_installed", onClose() {}, onSaved() {},
  }));
  await act(async () => { root = TestRenderer.create(tree()); });
  t.after(async () => { await act(async () => root.unmount()); globalThis.window = oldWindow; globalThis.document = oldDocument; delete globalThis.__adoptOrigin; });
  const adoptButtons = () => root.root.findAllByType("button").filter((button) => button.children.some((child) => typeof child === "string" && child.includes("接管")));
  return { root, requests, adoptButtons, get adoptions() { return adoptions; },
    update: async (id) => { await act(async () => root.update(tree(id))); },
    resolve: async (request, value) => { await act(async () => request.resolve(value)); },
  };
}

for (const [name, origin] of [["file import", { kind: "imported" }], ["legacy unknown", { kind: "migrated_v1" }], ["unknown", null], ["other migration", { ...migrated, adapterId: "other" }]]) {
  test(`${name} has no adoption UI while read-only checks remain available`, options, async (t) => {
    const h = await mount(t);
    assert.equal(h.adoptButtons().length, 0);
    await h.resolve(h.requests[0], detail(origin));
    assert.equal(h.adoptButtons().length, 0);
    assert.equal(h.root.root.findAll((node) => node.props.role === "alertdialog").length, 0);
    assert.equal(h.root.root.findAllByType("button").some((button) => button.children.some((child) => typeof child === "string" && child.includes("检查"))), true);
  });
}

test("only a loaded Hunting Box origin exposes adoption and requires confirmation", options, async (t) => {
  const h = await mount(t);
  await h.resolve(h.requests[0], detail(migrated));
  assert.equal(h.adoptButtons().length, 1);
  await act(async () => h.adoptButtons()[0].props.onClick());
  assert.equal(h.adoptions, 0);
  const confirmation = h.root.root.findByProps({ role: "alertdialog" });
  const button = confirmation.findAllByType("button").find((candidate) => candidate.children.some((child) => typeof child === "string" && child.includes("接管")));
  await act(async () => button.props.onClick());
  assert.equal(h.adoptions, 1);
});

test("changing mods cannot reuse the previous loaded migration origin", options, async (t) => {
  const h = await mount(t);
  await h.resolve(h.requests[0], detail(migrated));
  assert.equal(h.adoptButtons().length, 1);
  await h.update("b");
  assert.equal(h.adoptButtons().length, 0);
  await h.resolve(h.requests[1], detail({ kind: "imported" }, "b"));
  assert.equal(h.adoptButtons().length, 0);
});

test("failed or mismatched details never authorize adoption", options, async (t) => {
  const h = await mount(t);
  await h.resolve(h.requests[0], detail(migrated, "other"));
  assert.equal(h.adoptButtons().length, 0);
  await h.update("b");
  await act(async () => h.requests[1].reject(new Error("fixture failure")));
  assert.equal(h.adoptButtons().length, 0);
});
