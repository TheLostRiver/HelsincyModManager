import assert from "node:assert/strict";
import { test } from "node:test";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";
import { registerReactTestModules } from "../testing/reactModuleLoader.mjs";

registerReactTestModules({
  "shared/feedback/focusTrap.ts": `
    export const isTopmostModalSurface = (container) => container === globalThis.__modalFocus.topmost;
    export const getFocusableElements = () => [];
    export const getTrappedFocusIndex = () => -1;`,
});
const { useModalFocusTrap } = await import("./useModalFocusTrap.ts");
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

test("Escape consumed by a closing child cannot close a parent listener registered after it", async (t) => {
  const previous = { document: globalThis.document, window: globalThis.window, HTMLElement: globalThis.HTMLElement };
  const listeners = new Set();
  globalThis.document = { activeElement: null, addEventListener: (_type, listener) => listeners.add(listener), removeEventListener: (_type, listener) => listeners.delete(listener) };
  globalThis.window = { requestAnimationFrame: () => 1, cancelAnimationFrame: () => {} };
  globalThis.HTMLElement = class {};
  const parent = {}, child = {};
  globalThis.__modalFocus = { topmost: child };
  const closed = [];
  function Fixture() {
    useModalFocusTrap({ active: true, containerRef: { current: child }, closeOnEscape: true,
      onRequestClose: () => { closed.push("child"); globalThis.__modalFocus.topmost = parent; } });
    useModalFocusTrap({ active: true, containerRef: { current: parent }, closeOnEscape: true, onRequestClose: () => closed.push("parent") });
    return null;
  }
  let root;
  t.after(async () => {
    await act(async () => root?.unmount());
    for (const [key, value] of Object.entries(previous)) { if (value === undefined) delete globalThis[key]; else globalThis[key] = value; }
    delete globalThis.__modalFocus;
  });
  await act(async () => { root = TestRenderer.create(React.createElement(Fixture)); });
  const escape = () => ({ key: "Escape", defaultPrevented: false, preventDefault() { this.defaultPrevented = true; }, stopPropagation() {} });
  await act(async () => { const event = escape(); for (const listener of listeners) listener(event); });
  assert.deepEqual(closed, ["child"]);
  await act(async () => { const event = escape(); for (const listener of listeners) listener(event); });
  assert.deepEqual(closed, ["child", "parent"], "a separate Escape may close the parent");
});
