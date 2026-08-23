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
  assert.match(modal, /aria-label=\{resolvedCloseLabel\}/);
  assert.match(modal, /active:\s*phase\s*!==\s*"closed"/);
  assert.match(modal, /closeOnEscape:\s*closeOnEscape\s*&&\s*canClose\s*&&\s*phase\s*!==\s*"closing"/);

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

test("modal surface keeps mounted opening and closing phases in sync with feedback styles", () => {
  const modal = readSource("src/shared/feedback/ModalSurface.tsx");
  const css = readSource("src/shared/feedback/feedback.css");

  const transitionDurationMs = Number(
    modal.match(/const MODAL_TRANSITION_MS = (\d+);/)?.[1],
  );
  const reducedMotionDurationMs = Number(
    modal.match(/const REDUCED_MOTION_TRANSITION_MS = (\d+);/)?.[1],
  );
  assert.ok(Number.isInteger(transitionDurationMs) && transitionDurationMs > 0);
  assert.ok(Number.isInteger(reducedMotionDurationMs) && reducedMotionDurationMs > 0);
  assert.ok(reducedMotionDurationMs < transitionDurationMs);

  assert.match(modal, /type ModalPhase = "closed" \| "opening" \| "open" \| "settled" \| "closing"/);
  assert.match(
    modal,
    /window\.requestAnimationFrame\(\(\) => \{[\s\S]*window\.requestAnimationFrame\(\(\) => \{[\s\S]*updatePhase\("open"\)/,
  );
  assert.match(
    modal,
    /updatePhase\("closing"\)[\s\S]*window\.setTimeout\(\(\) => \{[\s\S]*updatePhase\("closed"\)/,
  );
  assert.ok(modal.includes('className={`feedback-overlay is-${kind} is-${phase}`}'));
  assert.ok(modal.includes('"--feedback-modal-transition-duration": `${getModalTransitionMillis()}ms`'));
  assert.match(modal, /onClickCapture=\{blockInteractionWhileClosing\}/);
  assert.match(modal, /onKeyDownCapture=\{blockInteractionWhileClosing\}/);
  assert.match(
    modal,
    /phase\s*!==\s*"closing"[\s\S]*event\.preventDefault\(\)[\s\S]*event\.stopPropagation\(\)/,
  );

  assert.match(
    css,
    new RegExp(`var\\(--feedback-modal-transition-duration, ${transitionDurationMs}ms\\)`),
  );
  assert.match(css, /\.feedback-overlay\.is-closing\s*\{[\s\S]*?pointer-events:\s*none/);
  assert.match(css, /\.feedback-overlay\.is-opening \.feedback-modal[\s\S]*?translateY\(8px\) scale\(0\.985\)/);

  const reducedMotionStyles = css.slice(css.indexOf("@media (prefers-reduced-motion: reduce)"));
  assert.match(reducedMotionStyles, /\.feedback-overlay\.is-opening \.feedback-modal[\s\S]*?transform:\s*none/);
  assert.match(reducedMotionStyles, /\.feedback-overlay\.is-open \.feedback-modal[\s\S]*?transition-property:\s*opacity/);
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

test("toast exit animation duration stays in sync with the component's removal delay", () => {
  const css = readSource("src/shared/feedback/feedback.css");
  const component = readSource("src/shared/feedback/FeedbackToast.tsx");

  /*
   * 退场是"先标记、等动画播完再卸载"两段式：组件按常量延迟移除节点，CSS 负责这段时间内的动画。
   * 两者一旦不一致，要么 toast 先消失再空等（看起来卡顿），要么动画被卸载打断（看起来仍然生硬）。
   * 这条隐性契约没有类型或运行时保护，因此在此显式锁定。
   */
  const exitDurationMs = Number(
    component.match(/const TOAST_EXIT_DURATION_MS = (\d+);/)?.[1],
  );
  assert.ok(Number.isInteger(exitDurationMs) && exitDurationMs > 0, "缺少 TOAST_EXIT_DURATION_MS 常量");

  const exitRules = [...css.matchAll(/\.feedback-toast\.is-exiting\s*\{([\s\S]*?)\}/g)].map(
    (match) => match[1],
  );
  assert.ok(exitRules.length > 0, "缺少 .feedback-toast.is-exiting 规则");

  for (const rule of exitRules) {
    const declaredMs = rule.match(/animation(?:-duration)?:[^;]*?(\d+)ms/)?.[1];
    if (declaredMs === undefined) {
      continue;
    }
    assert.equal(
      Number(declaredMs),
      exitDurationMs,
      `退场动画时长 ${declaredMs}ms 与组件常量 ${exitDurationMs}ms 不一致`,
    );
  }

  // 退场期间必须屏蔽交互，避免点到正在消失的按钮。
  assert.match(css, /\.feedback-toast\.is-exiting\s*\{[\s\S]*?pointer-events:\s*none/);
  // 动效降级下退场只做淡出，不带位移。
  assert.match(css, /@keyframes feedback-fade-exit/);
});
