import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import test from "node:test";

import { getTrappedFocusIndex } from "./focusTrap.ts";

function readSource(path) {
  assert.equal(existsSync(path), true, `missing feedback source: ${path}`);
  return readFileSync(path, "utf8");
}

test("shared focus trap wraps in both directions and handles an empty surface", () => {
  assert.equal(getTrappedFocusIndex({ currentIndex: 0, focusableCount: 3, backwards: true }), 2);
  assert.equal(getTrappedFocusIndex({ currentIndex: 2, focusableCount: 3, backwards: false }), 0);
  assert.equal(getTrappedFocusIndex({ currentIndex: -1, focusableCount: 3, backwards: false }), 0);
  assert.equal(getTrappedFocusIndex({ currentIndex: -1, focusableCount: 3, backwards: true }), 2);
  assert.equal(getTrappedFocusIndex({ currentIndex: -1, focusableCount: 0, backwards: false }), -1);
});

test("feedback provider owns one body-level portal host", () => {
  const provider = readSource("src/shared/feedback/FeedbackProvider.tsx");

  assert.match(provider, /document\.createElement\("div"\)/);
  assert.match(provider, /dataset\.feedbackHost\s*=\s*"true"/);
  assert.match(provider, /document\.body\.appendChild\(nextHost\)/);
  assert.match(provider, /nextHost\.remove\(\)/);
  assert.match(provider, /createPortal\(children, host\)/);
});

test("modal surface exposes dialog semantics, close policy, and shared focus behavior", () => {
  const modal = readSource("src/shared/feedback/ModalSurface.tsx");
  const focus = readSource("src/shared/feedback/useModalFocusTrap.ts");

  assert.match(modal, /role=\{role\}/);
  assert.match(modal, /aria-modal="true"/);
  assert.match(modal, /aria-labelledby=\{titleId\}/);
  assert.match(modal, /closeOnEscape:\s*closeOnEscape\s*&&\s*canClose/);
  assert.match(modal, /event\.target\s*===\s*event\.currentTarget\s*&&\s*closeOnBackdrop\s*&&\s*canClose/);
  assert.match(modal, /aria-label=\{closeLabel\}/);

  assert.match(focus, /document\.addEventListener\("keydown", handleKeyDown, true\)/);
  assert.match(focus, /event\.key\s*===\s*"Escape"\s*&&\s*closeOnEscape/);
  assert.match(focus, /getTrappedFocusIndex/);
  assert.match(
    focus,
    /event\.key\s*!==\s*"Escape"\s*&&\s*event\.key\s*!==\s*"Tab"[\s\S]*isTopmostModalSurface\(container\)[\s\S]*event\.key\s*===\s*"Escape"/,
  );
  assert.match(focus, /restoreFocusRef\.current\?\.isConnected/);
  assert.match(focus, /restoreFocusRef\.current\.focus\(\)/);

  const focusFrameStart = focus.indexOf("const frameId");
  const focusFrameEnd = focus.indexOf("return () =>", focusFrameStart);
  const focusFrame = focus.slice(focusFrameStart, focusFrameEnd);
  assert.match(focusFrame, /isTopmostModalSurface\(container\)[\s\S]*target\.focus\(\)/);
});

test("task and toast primitives expose stable live-region containers without business logic", () => {
  const taskNotice = readSource("src/shared/feedback/TaskNotice.tsx");
  const toastViewport = readSource("src/shared/feedback/ToastViewport.tsx");

  assert.match(taskNotice, /data-task-id=\{taskId\}/);
  assert.match(taskNotice, /role=\{role\}/);
  assert.match(taskNotice, /aria-live=\{tone\s*===\s*"danger"\s*\?\s*"assertive"\s*:\s*"polite"\}/);
  assert.doesNotMatch(taskNotice, /setTimeout|invoke\(|listen\(/);

  assert.match(toastViewport, /role="region"/);
  assert.match(toastViewport, /aria-live="polite"/);
  assert.match(toastViewport, /aria-relevant="additions removals"/);
  assert.doesNotMatch(toastViewport, /setTimeout|queue|invoke\(|listen\(/);
});

test("feedback styles keep stable layers and reduced-motion behavior", () => {
  const css = readSource("src/shared/feedback/feedback.css");
  const tokens = readSource("src/shared/styles/tokens.css");
  const hostRule = css.match(/\.feedback-host\s*\{([\s\S]*?)\}/)?.[1];

  for (const token of ["--z-feedback-task", "--z-feedback-toast", "--z-feedback-sheet", "--z-feedback-dialog"]) {
    assert.match(tokens, new RegExp(token));
  }
  assert.ok(hostRule);
  assert.match(hostRule, /pointer-events:\s*none/);
  assert.doesNotMatch(hostRule, /position|z-index|transform|isolation/);
  assert.match(css, /z-index:\s*var\(--z-feedback-dialog\)/);
  assert.match(css, /z-index:\s*var\(--z-feedback-sheet\)/);
  assert.match(css, /@media\s*\(prefers-reduced-motion:\s*reduce\)/);
  assert.match(css, /max-height:\s*calc\(100dvh\s*-\s*32px\)/);
});
