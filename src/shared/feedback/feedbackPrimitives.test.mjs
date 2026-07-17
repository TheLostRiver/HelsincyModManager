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
  assert.match(provider, /event\.key\s*!==\s*"Escape"/);
  assert.match(provider, /document\.querySelector\('\[aria-modal="true"\]'\)/);
  assert.match(provider, /queue\.slice\(0,\s*-1\)/);
  assert.match(provider, /enqueueFeedbackToast/);
  assert.match(provider, /showTaskNotice/);
  assert.match(provider, /findIndex\(\(notice\) => notice\.taskId === input\.taskId\)/);
  assert.match(provider, /next\[index\] = input/);
  assert.match(provider, /const actions = useMemo/);
  assert.match(provider, /<FeedbackActionsContext\.Provider value=\{actions\}>/);
  assert.match(provider, /taskNotices\.map\(\(notice\) => <TaskNotice key=\{notice\.taskId\}/);
});

test("shared toast pauses dismissal, supports one action, and carries stable source keys", () => {
  const toast = readSource("src/shared/feedback/FeedbackToast.tsx");
  const state = readSource("src/shared/feedback/feedbackToastState.ts");

  assert.match(toast, /data-event-key=\{toast\.eventKey\}/);
  assert.match(toast, /data-task-id=\{toast\.taskId\}/);
  assert.match(toast, /onMouseEnter=\{\(\) => setPaused\(true\)\}/);
  assert.match(toast, /onFocusCapture=\{\(\) => setPaused\(true\)\}/);
  assert.match(toast, /toast\.action\s*\?/);
  assert.match(state, /eventKey:\s*string/);
  assert.match(state, /action\?:\s*FeedbackToastAction/);
  assert.match(state, /findIndex\(\(toast\) => toast\.eventKey === input\.eventKey\)/);
  assert.doesNotMatch(state, /toast\.message === input\.message/);
});

test("modal surface exposes dialog semantics, close policy, and shared focus behavior", () => {
  const modal = readSource("src/shared/feedback/ModalSurface.tsx");
  const focus = readSource("src/shared/feedback/useModalFocusTrap.ts");
  const focusTrap = readSource("src/shared/feedback/focusTrap.ts");

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
  assert.match(focus, /activeModal\s*&&\s*activeModal\s*!==\s*containerAtActivation/);

  assert.match(focusTrap, /getClientRects\(\)\.length\s*===\s*0/);
  assert.match(focusTrap, /hasAttribute\("hidden"\)/);
  assert.match(focusTrap, /hasAttribute\("inert"\)/);
  assert.match(focusTrap, /getAttribute\("aria-hidden"\)\s*===\s*"true"/);
  assert.match(focusTrap, /style\.display\s*===\s*"none"/);
  assert.match(focusTrap, /style\.visibility\s*===\s*"hidden"/);

  const focusFrameStart = focus.indexOf("const frameId");
  const focusFrameEnd = focus.indexOf("return () =>", focusFrameStart);
  const focusFrame = focus.slice(focusFrameStart, focusFrameEnd);
  assert.match(focusFrame, /isTopmostModalSurface\(container\)[\s\S]*target\.focus\(\)/);
});

test("task and toast primitives expose stable live-region containers without business logic", () => {
  const taskNotice = readSource("src/shared/feedback/TaskNotice.tsx");
  const taskViewport = readSource("src/shared/feedback/TaskNoticeViewport.tsx");
  const toastViewport = readSource("src/shared/feedback/ToastViewport.tsx");

  assert.match(taskNotice, /data-task-id=\{taskId\}/);
  assert.match(taskNotice, /role=\{role\}/);
  assert.match(taskNotice, /aria-live=\{tone\s*===\s*"danger"\s*\?\s*"assertive"\s*:\s*"polite"\}/);
  assert.doesNotMatch(taskNotice, /FeedbackPortal|setTimeout|queue|invoke\(|listen\(/);

  assert.match(taskViewport, /FeedbackPortal/);
  assert.match(taskViewport, /className="feedback-task-notice-viewport"/);
  assert.match(taskViewport, /role="region"/);
  assert.doesNotMatch(taskViewport, /setTimeout|queue|invoke\(|listen\(/);

  assert.match(toastViewport, /role="region"/);
  assert.match(toastViewport, /aria-live="polite"/);
  assert.match(toastViewport, /aria-relevant="additions removals"/);
  assert.doesNotMatch(toastViewport, /setTimeout|queue|invoke\(|listen\(/);
});

test("feedback styles keep stable layers and reduced-motion behavior", () => {
  const css = readSource("src/shared/feedback/feedback.css");
  const tokens = readSource("src/shared/styles/tokens.css");
  const hostRule = css.match(/\.feedback-host\s*\{([\s\S]*?)\}/)?.[1];
  const taskViewportRule = css.match(/\.feedback-task-notice-viewport\s*\{([\s\S]*?)\}/)?.[1];
  const taskNoticeRule = css.match(/\.feedback-task-notice\s*\{([\s\S]*?)\}/)?.[1];

  for (const token of ["--z-feedback-task", "--z-feedback-toast", "--z-feedback-sheet", "--z-feedback-dialog"]) {
    assert.match(tokens, new RegExp(token));
  }
  assert.ok(hostRule);
  assert.ok(taskViewportRule);
  assert.ok(taskNoticeRule);
  assert.match(hostRule, /pointer-events:\s*none/);
  assert.doesNotMatch(hostRule, /position|z-index|transform|isolation/);
  assert.match(taskViewportRule, /position:\s*fixed/);
  assert.match(taskViewportRule, /display:\s*grid/);
  assert.match(taskViewportRule, /gap:\s*12px/);
  assert.doesNotMatch(taskNoticeRule, /position:\s*fixed|right:|bottom:|z-index:/);
  assert.match(css, /z-index:\s*var\(--z-feedback-dialog\)/);
  assert.match(css, /z-index:\s*var\(--z-feedback-sheet\)/);
  assert.match(css, /@media\s*\(prefers-reduced-motion:\s*reduce\)/);
  assert.match(css, /max-height:\s*calc\(100dvh\s*-\s*32px\)/);
});
