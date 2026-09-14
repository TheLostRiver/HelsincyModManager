import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "shared/i18n/index.ts": `export { resolveCopy } from "./locales.ts"; export const useI18n = () => ({ locale: globalThis.__retargetWorkspaceLocale ?? "zh_cn" });`,
});
const { RetargetWorkspace } = await import("./RetargetWorkspace.tsx");
const { RetargetFileDetails } = await import("./RetargetFileDetails.tsx");
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

test("preview completion reveals and focuses file changes without submitting an install", async (t) => {
  let installs = 0;
  const focusCalls = [];
  const previewPane = { scrollTop: 420, focus: (options) => focusCalls.push(options) };
  const tree = (previewStatus) => React.createElement(RetargetWorkspace, {
    previewStatus,
    selection: React.createElement("input", { defaultValue: "Selected target" }),
    preview: React.createElement("p", null, previewStatus === "error" ? "Preview failed" : "File changes"),
    feedback: null,
    actions: React.createElement("button", { disabled: previewStatus !== "ready", onClick: () => installs++ }, "Confirm"),
  });
  let root;
  await act(async () => {
    root = TestRenderer.create(tree("idle"), { createNodeMock: (element) => element.props.className === "retarget-workspace__preview" ? previewPane : null });
  });
  t.after(async () => { await act(async () => root.unmount()); delete globalThis.__retargetWorkspaceLocale; });
  const pane = () => root.root.findByProps({ className: "retarget-workspace__preview" });
  const confirm = () => root.root.findByProps({ className: "replacement-panel__actions" }).findByType("button");
  assert.equal(pane().props["data-active"], false);
  await act(async () => root.update(tree("loading")));
  assert.equal(pane().props["data-active"], true);
  assert.equal(confirm().props.disabled, true);
  await act(async () => root.update(tree("ready")));
  assert.equal(previewPane.scrollTop, 0);
  assert.deepEqual(focusCalls, [{ preventScroll: true }]);
  assert.equal(confirm().props.disabled, false);
  assert.equal(installs, 0);

  previewPane.scrollTop = 250;
  globalThis.__retargetWorkspaceLocale = "en";
  await act(async () => root.update(tree("ready")));
  assert.equal(previewPane.scrollTop, 250, "ordinary rerenders must not interrupt reading");
  assert.equal(focusCalls.length, 1);
  await act(async () => root.update(tree("error")));
  assert.equal(previewPane.scrollTop, 0);
  assert.equal(confirm().props.disabled, true);
  assert.equal(installs, 0);
});

test("retarget file details can open with the preview and still collapse or filter", async (t) => {
  const file = { fileId: "model", sourceId: null, sourcePath: "nativePC/fixture/source.mod3", installedPath: null,
    targetPath: "nativePC/fixture/target.mod3", disposition: "relocated", reason: "target_mapping", change: "added" };
  let root;
  await act(async () => { root = TestRenderer.create(React.createElement(RetargetFileDetails, { files: [file], defaultOpen: true })); });
  t.after(async () => { await act(async () => root.unmount()); });
  assert.equal(root.root.findByType("details").props.open, true);
  assert.equal(root.root.findAllByType("li").length, 1);
  await act(async () => root.root.findByType("input").props.onChange({ target: { value: "no matching file" } }));
  assert.equal(root.root.findAllByType("li").length, 0);
  await act(async () => root.root.findByType("details").props.onToggle({ currentTarget: { open: false } }));
  assert.equal(root.root.findByType("details").props.open, false);
  assert.equal(root.root.findAllByType("input").length, 0);
});
