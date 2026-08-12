import assert from "node:assert/strict";
import { test } from "node:test";

import {
  BACKGROUND_PROTECTION_AUTO_VERIFICATION_DELAYS_MS,
  BackgroundProtectionAutoVerificationScheduler,
} from "./backgroundProtectionAutoVerification.ts";

function createTimerHarness() {
  let nextId = 1;
  const timers = new Map();

  return {
    setTimer(callback, delayMs) {
      const id = nextId++;
      timers.set(id, { callback, delayMs });
      return id;
    },
    clearTimer(id) {
      timers.delete(id);
    },
    nextDelay() {
      return timers.values().next().value?.delayMs ?? null;
    },
    size() {
      return timers.size;
    },
    async runNext() {
      const entry = timers.entries().next().value;
      assert.ok(entry, "a timer should be scheduled");
      const [id, timer] = entry;
      timers.delete(id);
      timer.callback();
      await new Promise((resolve) => setImmediate(resolve));
    },
  };
}

test("auto verification runs almost immediately and stops after convergence", async () => {
  const timers = createTimerHarness();
  const decisions = ["continue", "complete"];
  const activeChanges = [];
  let verifyCount = 0;
  const scheduler = new BackgroundProtectionAutoVerificationScheduler({
    verify: async () => {
      verifyCount += 1;
      return decisions.shift();
    },
    isBusy: () => false,
    onActiveChange: (active) => activeChanges.push(active),
    setTimer: timers.setTimer,
    clearTimer: timers.clearTimer,
  });

  scheduler.arm();
  assert.equal(timers.nextDelay(), 750);
  assert.equal(BACKGROUND_PROTECTION_AUTO_VERIFICATION_DELAYS_MS[0], 750);

  await timers.runNext();
  assert.equal(verifyCount, 1);
  assert.equal(timers.nextDelay(), 59_250);

  await timers.runNext();
  assert.equal(verifyCount, 2);
  assert.equal(timers.size(), 0);
  assert.equal(scheduler.isActive(), false);
  assert.deepEqual(activeChanges, [true, false]);
});

test("temporary verification failures preserve the remaining retry points", async () => {
  const timers = createTimerHarness();
  let verifyCount = 0;
  const scheduler = new BackgroundProtectionAutoVerificationScheduler({
    verify: async () => {
      verifyCount += 1;
      if (verifyCount === 1) throw new Error("temporary read failure");
      return "complete";
    },
    isBusy: () => false,
    onActiveChange: () => {},
    delaysMs: [10, 20],
    setTimer: timers.setTimer,
    clearTimer: timers.clearTimer,
  });

  scheduler.arm();
  await timers.runNext();
  assert.equal(timers.nextDelay(), 20);
  await timers.runNext();
  assert.equal(verifyCount, 2);
  assert.equal(scheduler.isActive(), false);
});

test("a busy operation defers verification without consuming a retry point", async () => {
  const timers = createTimerHarness();
  let busy = true;
  let verifyCount = 0;
  const scheduler = new BackgroundProtectionAutoVerificationScheduler({
    verify: async () => {
      verifyCount += 1;
      return "complete";
    },
    isBusy: () => busy,
    onActiveChange: () => {},
    delaysMs: [10],
    busyRetryDelayMs: 25,
    setTimer: timers.setTimer,
    clearTimer: timers.clearTimer,
  });

  scheduler.arm();
  await timers.runNext();
  assert.equal(verifyCount, 0);
  assert.equal(timers.nextDelay(), 25);

  busy = false;
  await timers.runNext();
  assert.equal(verifyCount, 1);
  assert.equal(scheduler.isActive(), false);
});

test("disposing the scheduler cancels page-scoped verification", async () => {
  const timers = createTimerHarness();
  let verifyCount = 0;
  const scheduler = new BackgroundProtectionAutoVerificationScheduler({
    verify: async () => {
      verifyCount += 1;
      return "complete";
    },
    isBusy: () => false,
    onActiveChange: () => {},
    setTimer: timers.setTimer,
    clearTimer: timers.clearTimer,
  });

  scheduler.arm();
  scheduler.dispose();
  assert.equal(timers.size(), 0);
  assert.equal(scheduler.isActive(), false);
  assert.equal(verifyCount, 0);
});
