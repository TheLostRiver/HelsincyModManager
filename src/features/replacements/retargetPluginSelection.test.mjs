import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "shared/i18n/index.ts": `export { resolveCopy } from "./locales.ts"; export const useI18n = () => ({ locale: "zh_cn" });`,
});
const { RetargetPluginSelection } = await import("./RetargetPluginSelection.tsx");
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const text = (node) => typeof node === "string" ? node : Array.isArray(node) ? node.map(text).join("") : (node?.children ?? []).map(text).join("");

test("attachment counts stay compact while read/save failures and retry remain visible", async (t) => {
  let reloads = 0;
  const controller = { status: "ready", inventory: { files: [{ selected: true }, { selected: false }, { selected: false }] },
    error: null, saving: false, draft: false, reload: () => reloads++, choose: () => { throw new Error("opening details must not save a choice"); } };
  let root;
  const tree = () => React.createElement(RetargetPluginSelection, { controller, disabled: false });
  await act(async () => { root = TestRenderer.create(tree()); });
  t.after(async () => { await act(async () => root.unmount()); });
  assert.ok(text(root.toJSON()).includes("附件 · 已选 1 / 排除 2"));
  assert.equal(root.root.findAllByProps({ type: "checkbox" }).length, 0);
  assert.equal(reloads, 0);
  controller.error = { code: "plugin_inventory_changed" };
  await act(async () => root.update(tree()));
  assert.ok(text(root.root.findByProps({ role: "alert" })).includes("包内容、版本或选择已变化"));
  await act(async () => root.root.findByProps({ role: "alert" }).findByType("button").props.onClick());
  assert.equal(reloads, 1);
  controller.inventory = null;
  controller.status = "failed";
  controller.error = { code: "plugin_selection_unavailable" };
  await act(async () => root.update(tree()));
  assert.ok(text(root.root.findByProps({ role: "alert" })).includes("插件选择暂时不可用"));
  assert.equal(root.root.findAllByProps({ className: "retarget-popover__trigger" }).length, 0);
  controller.saving = true;
  await act(async () => root.update(tree()));
  assert.equal(root.root.findByProps({ role: "alert" }).findByType("button").props.disabled, true);
  assert.ok(text(root.root.findByProps({ role: "status" })).includes("正在保存选择"));
});
