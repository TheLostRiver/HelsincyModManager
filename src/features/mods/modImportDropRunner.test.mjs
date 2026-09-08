import assert from "node:assert/strict";
import test from "node:test";

import { ModImportTaskWatcher, runDropImportPump } from "./modImportDropRunner.ts";

const progress = (taskId, status, phase = "mod_import.prepare.completed", extra = {}) => ({
  taskId,
  kind: "mod_import",
  status,
  phase,
  current: null,
  total: null,
  message: null,
  error: null,
  ...extra,
});

/**
 * 等一个终态，但**不无限等**。
 *
 * 丢事件的回归必须转红，而不是把测试挂住：node --test 默认没有超时，
 * 一个永不 resolve 的 await 会让整个套件（连同 CI）卡死，比失败还难查。
 * 反向验证正是这么撞上的——突变施加成功了，测试却不是转红而是挂住。
 */
function settleWithin(promise, ms = 500) {
  let timer;
  return Promise.race([
    promise.then((value) => {
      clearTimeout(timer);
      return value;
    }),
    new Promise((resolve) => {
      timer = setTimeout(() => resolve("<never-settled>"), ms);
    }),
  ]);
}

/**
 * 一个可追加的假队列 + 记账，形状与 Provider 里那份一致。
 *
 * `takeNext` **同步**，且「取到 null」与「清标志」在同一 tick——这正是被测的不变量。
 */
function harness({ outcomeFor = () => "completed", startBehavior } = {}) {
  const queue = [];
  const watcher = new ModImportTaskWatcher();
  const started = [];
  const settled = [];
  let inFlight = 0;
  let overlapped = false;
  let taskSeq = 0;
  let pumpRunning = false;

  const deps = {
    watcher,
    takeNext: () => {
      const next = queue.shift() ?? null;
      if (next === null) pumpRunning = false;
      return next;
    },
    startImport: async (archivePath) => {
      if (inFlight > 0) overlapped = true;
      inFlight += 1;
      if (startBehavior) {
        const forced = await startBehavior(archivePath);
        if (forced) {
          inFlight -= 1;
          return forced;
        }
      }
      taskSeq += 1;
      const taskId = `t${taskSeq}`;
      queueMicrotask(() => {
        inFlight -= 1;
        watcher.handleProgress(progress(taskId, outcomeFor(archivePath)));
      });
      return { kind: "mod_import", status: "queued", taskId };
    },
    onStarted: (archivePath) => started.push(archivePath),
    onSettled: (archivePath, outcome) => settled.push([archivePath, outcome]),
  };

  let pumpPromise = Promise.resolve();
  function enqueue(...paths) {
    queue.push(...paths);
    if (pumpRunning) return;
    pumpRunning = true;
    pumpPromise = pumpPromise.then(() =>
      runDropImportPump(deps).finally(() => {
        pumpRunning = false;
      }),
    );
  }

  return {
    watcher,
    started,
    settled,
    enqueue,
    drain: () => pumpPromise,
    get overlapped() {
      return overlapped;
    },
  };
}

// ---- 终态识别 ----

test("完成算成功，失败算失败", { timeout: 5_000 }, async () => {
  const watcher = new ModImportTaskWatcher();
  watcher.beginStart();
  const good = watcher.watch("t1");
  const bad = watcher.watch("t2");
  watcher.endStart();

  watcher.handleProgress(progress("t1", "completed"));
  watcher.handleProgress(progress("t2", "failed"));

  assert.equal(await settleWithin(good), "succeeded");
  assert.equal(await settleWithin(bad), "failed");
});

test("取消算失败，不算成功", { timeout: 5_000 }, async () => {
  // 取消之后库里确实没多出这个 Mod，报成功就是骗人。
  const watcher = new ModImportTaskWatcher();
  const outcome = watcher.watch("t1");
  watcher.handleProgress(progress("t1", "cancelled", "mod_import.cancelled"));
  assert.equal(await settleWithin(outcome), "failed");
});

test("taskId 已知之前到达的进度事件不会丢", { timeout: 5_000 }, async () => {
  // 真实存在的竞态：一个瞬间跑完的导入，终态事件会早于 start 的返回值到达。
  const watcher = new ModImportTaskWatcher();
  watcher.beginStart();
  watcher.handleProgress(progress("t1", "running", "mod_import.unpack.started"));
  watcher.handleProgress(progress("t1", "completed"));

  const outcome = watcher.watch("t1");
  watcher.endStart();
  assert.equal(await settleWithin(outcome), "succeeded", "缓存下来的终态必须在认领时补放");
});

test("start 结束之后不再缓存，陈旧事件不污染下一次", { timeout: 5_000 }, async () => {
  const watcher = new ModImportTaskWatcher();
  watcher.beginStart();
  watcher.endStart();
  watcher.handleProgress(progress("stray", "completed"));

  const outcome = watcher.watch("stray");
  let done = false;
  void outcome.then(() => {
    done = true;
  });
  await Promise.resolve();
  assert.equal(done, false);
});

test("别的任务种类永远不会结掉一个导入", { timeout: 5_000 }, async () => {
  const watcher = new ModImportTaskWatcher();
  const outcome = watcher.watch("t1");
  watcher.handleProgress(
    progress("t1", "completed", "mod_import.prepare.completed", { kind: "mod_install" }),
  );

  let done = false;
  void outcome.then(() => {
    done = true;
  });
  await Promise.resolve();
  assert.equal(done, false);

  watcher.handleProgress(progress("t1", "completed"));
  assert.equal(await settleWithin(outcome), "succeeded");
});

// ---- 泵：串行、可追加 ----

test("按入队顺序逐个跑，任一时刻只有一个在飞", { timeout: 5_000 }, async () => {
  const h = harness();
  h.enqueue("a", "b", "c");
  await h.drain();

  assert.equal(h.overlapped, false, "上一个还没出终态就起了下一个");
  assert.deepEqual(h.started, ["a", "b", "c"]);
  assert.deepEqual(h.settled, [
    ["a", "succeeded"],
    ["b", "succeeded"],
    ["c", "succeeded"],
  ]);
});

test("跑着的时候追加，新项会被同一个泵接着跑完", { timeout: 5_000 }, async () => {
  // 这是「导入中还能继续拖」的核心：队列不是开跑那一刻的快照。
  const h = harness();
  h.enqueue("a");
  // 不等 drain，立刻追加——模拟玩家在导入过程中又拖了两个进来。
  h.enqueue("b");
  h.enqueue("c");
  await h.drain();

  assert.deepEqual(h.started, ["a", "b", "c"], "追加的项必须被跑到");
  assert.equal(h.settled.length, 3);
});

test("泵跑空之后再入队，会被重新唤醒", { timeout: 5_000 }, async () => {
  // 上一轮结束与下一次入队之间不能有空窗，否则新项永远躺在队列里没人管。
  const h = harness();
  h.enqueue("a");
  await h.drain();
  assert.deepEqual(h.started, ["a"]);

  h.enqueue("b");
  await h.drain();
  assert.deepEqual(h.started, ["a", "b"], "跑空之后入队必须能重新起来");
});

test("一个失败不打断后面的", { timeout: 5_000 }, async () => {
  // 批量导入里一个坏包让后面的都不跑，是最容易犯也最难解释的错。
  const h = harness({ outcomeFor: (path) => (path === "b" ? "failed" : "completed") });
  h.enqueue("a", "b", "c");
  await h.drain();

  assert.deepEqual(h.started, ["a", "b", "c"]);
  assert.deepEqual(h.settled, [
    ["a", "succeeded"],
    ["b", "failed"],
    ["c", "succeeded"],
  ]);
});

test("起不来的任务算失败，队列继续", { timeout: 5_000 }, async () => {
  const h = harness({
    startBehavior: async (path) => {
      if (path === "a") throw new Error("storage frozen");
      return null;
    },
  });
  h.enqueue("a", "b");
  await h.drain();

  assert.deepEqual(h.settled, [
    ["a", "failed"],
    ["b", "succeeded"],
  ]);
});

test("start 返回异常状态时当场判失败，而不是挂住", { timeout: 5_000 }, async () => {
  // 后端若返回非 queued 的状态，就没有任务会发进度事件；不当场判失败的话，
  // 整个队列会永远停在「正在导入」。
  const h = harness({
    startBehavior: async () => ({ kind: "mod_import", status: "rejected", taskId: "x" }),
  });
  h.enqueue("a");
  await h.drain();

  assert.deepEqual(h.settled, [["a", "failed"]]);
});

test("队列空时泵立刻退出，不空转", { timeout: 5_000 }, async () => {
  let taken = 0;
  await runDropImportPump({
    watcher: new ModImportTaskWatcher(),
    takeNext: () => {
      taken += 1;
      return null;
    },
    startImport: async () => {
      throw new Error("不该被调用");
    },
    onStarted: () => assert.fail("空队列不该起任务"),
    onSettled: () => assert.fail("空队列不该有结果"),
  });
  assert.equal(taken, 1);
});
