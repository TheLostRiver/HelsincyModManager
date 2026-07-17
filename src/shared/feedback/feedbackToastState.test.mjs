import assert from "node:assert/strict";
import test from "node:test";

import {
  DEFAULT_FEEDBACK_TOAST_DURATION_MS,
  FEEDBACK_TOAST_QUEUE_LIMIT,
  dismissFeedbackToast,
  enqueueFeedbackToast,
} from "./feedbackToastState.ts";

test("toast queue merges by stable event key instead of message text", () => {
  const first = enqueueFeedbackToast([], {
    eventKey: "profile.discovery.failed",
    title: "检测失败",
    message: "第一次消息",
    taskId: "task-a",
  }, 1);
  const merged = enqueueFeedbackToast(first, {
    eventKey: "profile.discovery.failed",
    title: "再次检测失败",
    message: "第二次消息",
    taskId: "task-b",
    tone: "warning",
  }, 2);
  const sameCopyDifferentEvent = enqueueFeedbackToast(merged, {
    eventKey: "profile.discovery.other",
    title: "再次检测失败",
    message: "第二次消息",
  }, 3);

  assert.equal(merged.length, 1);
  assert.equal(merged[0].id, first[0].id);
  assert.equal(merged[0].occurrences, 2);
  assert.equal(merged[0].revision, 1);
  assert.equal(merged[0].taskId, "task-b");
  assert.equal(merged[0].message, "第二次消息");
  assert.equal(merged[0].durationMs, DEFAULT_FEEDBACK_TOAST_DURATION_MS);
  assert.equal(sameCopyDifferentEvent.length, 2);
});

test("toast queue keeps its limit and evicts the oldest item", () => {
  let queue = [];
  for (let sequence = 1; sequence <= FEEDBACK_TOAST_QUEUE_LIMIT + 1; sequence += 1) {
    queue = enqueueFeedbackToast(queue, {
      eventKey: `event-${sequence}`,
      title: `通知 ${sequence}`,
      message: "消息",
    }, sequence);
  }

  assert.equal(queue.length, FEEDBACK_TOAST_QUEUE_LIMIT);
  assert.equal(queue.some((toast) => toast.eventKey === "event-1"), false);
  assert.equal(queue.at(-1)?.eventKey, `event-${FEEDBACK_TOAST_QUEUE_LIMIT + 1}`);
});

test("dismiss removes only the requested toast", () => {
  const first = enqueueFeedbackToast([], { eventKey: "one", title: "一", message: "一" }, 1);
  const queue = enqueueFeedbackToast(first, { eventKey: "two", title: "二", message: "二" }, 2);

  assert.deepEqual(dismissFeedbackToast(queue, queue[0].id).map((toast) => toast.eventKey), ["two"]);
});
