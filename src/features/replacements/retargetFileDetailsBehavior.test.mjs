import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules({
  "shared/i18n/index.ts": `export { resolveCopy } from "./locales.ts"; export const useI18n = () => ({ locale: globalThis.__fileDetailsLocale });`,
});
const { RetargetFileDetails } = await import("./RetargetFileDetails.tsx");
const { retargetFileCopy } = await import("./retargetFileCopy.ts");
globalThis.IS_REACT_ACT_ENVIRONMENT = true;
const text = (node) => typeof node === "string" ? node : Array.isArray(node) ? node.map(text).join("") : (node?.children ?? []).map(text).join("");

test("file details expand on demand, paginate, search and translate without inferring paths", { concurrency: false }, async (t) => {
  const files = Array.from({ length: 205 }, (_, index) => ({ fileId: `fixture-${index}`, sourceId: "source",
    sourcePath: `nativePC/fixture/original/item-${index}.bin`, installedPath: `nativePC/fixture/current/item-${index}.bin`, targetPath: `nativePC/fixture/next/item-${index}.bin`,
    disposition: "relocated", reason: "target_mapping", change: "added" }));
  globalThis.__fileDetailsLocale = "zh_cn";
  const tree = () => React.createElement(RetargetFileDetails, { files, sourceLabels: { source: "Fixture equipment" } });
  let root;
  await act(async () => { root = TestRenderer.create(tree()); });
  t.after(async () => { await act(async () => root.unmount()); delete globalThis.__fileDetailsLocale; });
  assert.equal(root.root.findAllByType("li").length, 0);
  await act(async () => root.root.findByType("details").props.onToggle({ currentTarget: { open: true } }));
  assert.equal(root.root.findAllByType("li").length, 100);
  const first = text(root.root.findAllByType("li")[0]);
  for (const path of [files[0].sourcePath, files[0].installedPath, files[0].targetPath]) assert.ok(first.includes(path));
  await act(async () => root.root.findByType("button").props.onClick());
  assert.equal(root.root.findAllByType("li").length, 200);
  await act(async () => root.root.findByType("input").props.onChange({ target: { value: "item-204" } }));
  assert.equal(root.root.findAllByType("li").length, 1);
  await act(async () => root.root.findByType("input").props.onChange({ target: { value: "" } }));
  assert.equal(root.root.findAllByType("li").length, 100);
  for (const locale of ["en", "ja"]) {
    globalThis.__fileDetailsLocale = locale;
    await act(async () => root.update(tree()));
    assert.ok(text(root.toJSON()).includes(retargetFileCopy[locale].title(205)));
    assert.ok(text(root.toJSON()).includes(retargetFileCopy[locale].reasons.target_mapping));
  }
  await act(async () => root.root.findByType("details").props.onToggle({ currentTarget: { open: false } }));
  assert.equal(root.root.findAllByType("li").length, 0);
});

test("old responses without file details render nothing", { concurrency: false }, async (t) => {
  globalThis.__fileDetailsLocale = "zh_cn";
  let root;
  await act(async () => { root = TestRenderer.create(React.createElement(RetargetFileDetails)); });
  t.after(async () => { await act(async () => root.unmount()); delete globalThis.__fileDetailsLocale; });
  assert.equal(root.toJSON(), null);
});

test("ambiguous and conflicting identities explain why the entire path stays in place", { concurrency: false }, async (t) => {
  const reasons = ["ambiguous_resource_identity", "conflicting_resource_identity"];
  const files = reasons.map((reason, index) => ({ fileId: `kept-${index}`, sourceId: "source",
    sourcePath: `nativePC/fixture/kept-${index}.mod3`, targetPath: `nativePC/fixture/kept-${index}.mod3`,
    installedPath: null, disposition: "kept_in_place", reason, change: null }));
  globalThis.__fileDetailsLocale = "zh_cn";
  const tree = () => React.createElement(RetargetFileDetails, { files });
  let root;
  await act(async () => { root = TestRenderer.create(tree()); });
  t.after(async () => { await act(async () => root.unmount()); delete globalThis.__fileDetailsLocale; });
  await act(async () => root.root.findByType("details").props.onToggle({ currentTarget: { open: true } }));
  for (const locale of ["zh_cn", "en", "ja"]) {
    globalThis.__fileDetailsLocale = locale;
    await act(async () => root.update(tree()));
    for (const reason of reasons) assert.ok(text(root.toJSON()).includes(retargetFileCopy[locale].reasons[reason]));
    for (const file of files) assert.ok(text(root.toJSON()).includes(file.sourcePath));
  }
});
