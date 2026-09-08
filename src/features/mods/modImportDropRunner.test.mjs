import assert from "node:assert/strict";
import test from "node:test";

import { ModImportTaskWatcher, runDropImportBatch } from "./modImportDropRunner.ts";
import { dropRowsFromPreviews } from "./modImportDropState.ts";

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

const preview = (fileName, errorCode = null) => ({
  archivePath: `C:\\downloads\\${fileName}`,
  fileName,
  sizeBytes: 1024,
  warningCode: null,
  errorCode,
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

/** 起任务的假实现：按调用顺序发 taskId，并把每次调用记下来。 */
function fakeStarter(taskIds) {
  const calls = [];
  let index = 0;
  return {
    calls,
    start: async (archivePath) => {
      calls.push(archivePath);
      const taskId = taskIds[index++];
      return { kind: "mod_import", status: "queued", taskId };
    },
  };
}

test("a completed task resolves as succeeded, a failed one as failed", { timeout: 5_000 }, async () => {
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

test("a cancelled task counts as failed, not as success", { timeout: 5_000 }, async () => {
  // 取消之后库里确实没多出这个 Mod，报成功就是骗人。
  const watcher = new ModImportTaskWatcher();
  const outcome = watcher.watch("t1");
  watcher.handleProgress(progress("t1", "cancelled", "mod_import.cancelled"));
  assert.equal(await settleWithin(outcome), "failed");
});

test("progress that arrives before the task id is known is not lost", { timeout: 5_000 }, async () => {
  // 这是真实存在的竞态：一个瞬间跑完的导入，终态事件会早于 start 的返回值到达。
  const watcher = new ModImportTaskWatcher();
  watcher.beginStart();
  watcher.handleProgress(progress("t1", "running", "mod_import.unpack.started"));
  watcher.handleProgress(progress("t1", "completed"));

  const outcome = watcher.watch("t1");
  watcher.endStart();
  assert.equal(await settleWithin(outcome), "succeeded", "缓存下来的终态必须在认领时补放");
});

test("events for tasks nobody is watching are dropped once the start settles", { timeout: 5_000 }, async () => {
  const watcher = new ModImportTaskWatcher();
  watcher.beginStart();
  watcher.endStart();
  watcher.handleProgress(progress("stray", "completed"));

  // 认领一个同名 task 不该拿到刚才那条陈旧事件的结果。
  const outcome = watcher.watch("stray");
  let settled = false;
  void outcome.then(() => {
    settled = true;
  });
  await Promise.resolve();
  assert.equal(settled, false, "endStart 之后不再缓存，陈旧事件不能污染下一次");
});

test("progress from other task kinds never settles a mod import", { timeout: 5_000 }, async () => {
  const watcher = new ModImportTaskWatcher();
  const outcome = watcher.watch("t1");
  watcher.handleProgress(progress("t1", "completed", "mod_import.prepare.completed", {
    kind: "mod_install",
  }));

  let settled = false;
  void outcome.then(() => {
    settled = true;
  });
  await Promise.resolve();
  assert.equal(settled, false);

  watcher.handleProgress(progress("t1", "completed"));
  assert.equal(await settleWithin(outcome), "succeeded");
});

test("the batch imports one at a time, in list order", { timeout: 5_000 }, async () => {
  const rows = dropRowsFromPreviews([preview("a.zip"), preview("b.rar"), preview("c.7z")]);
  const watcher = new ModImportTaskWatcher();
  const starter = fakeStarter(["t1", "t2", "t3"]);
  const seen = [];

  // 「串行」的可证伪判据：任一时刻只允许有一个导入在飞。
  // **不在回调里直接 assert**——runOne 用 try/catch 兜住了起任务的失败，
  // 抛在里面的断言会被吞成「这一条失败」，绿灯就不承重了。记违例、最后断。
  let inFlight = 0;
  let overlapped = false;
  const run = await runDropImportBatch(rows, {
    watcher,
    startImport: async (path) => {
      if (inFlight > 0) overlapped = true;
      inFlight += 1;
      const task = await starter.start(path);
      queueMicrotask(() => {
        inFlight -= 1;
        watcher.handleProgress(progress(task.taskId, "completed"));
      });
      return task;
    },
    onRunChange: (next) => seen.push(next.currentPath),
    shouldStop: () => false,
  });

  assert.equal(overlapped, false, "上一个还没出终态就起了下一个");
  assert.deepEqual(starter.calls, rows.map((row) => row.archivePath));
  assert.equal(run.currentPath, null);
  assert.deepEqual(Object.values(run.results), ["succeeded", "succeeded", "succeeded"]);
});

test("blocked rows are never started", { timeout: 5_000 }, async () => {
  const rows = dropRowsFromPreviews([
    preview("good.zip"),
    preview("secret.rar", "mod_import_archive_encrypted"),
  ]);
  const watcher = new ModImportTaskWatcher();
  const starter = fakeStarter(["t1"]);

  await runDropImportBatch(rows, {
    watcher,
    startImport: async (path) => {
      const task = await starter.start(path);
      queueMicrotask(() => watcher.handleProgress(progress(task.taskId, "completed")));
      return task;
    },
    onRunChange: () => {},
    shouldStop: () => false,
  });

  assert.deepEqual(starter.calls, [rows[0].archivePath], "读不了的包不该被起任务");
});

test("one failure does not abort the rest of the batch", { timeout: 5_000 }, async () => {
  // 批量导入里一个坏包让后面的都不跑，是最容易犯也最难解释的错。
  const rows = dropRowsFromPreviews([preview("a.zip"), preview("b.zip"), preview("c.zip")]);
  const watcher = new ModImportTaskWatcher();
  const starter = fakeStarter(["t1", "t2", "t3"]);

  const run = await runDropImportBatch(rows, {
    watcher,
    startImport: async (path) => {
      const task = await starter.start(path);
      const status = task.taskId === "t2" ? "failed" : "completed";
      queueMicrotask(() => watcher.handleProgress(progress(task.taskId, status)));
      return task;
    },
    onRunChange: () => {},
    shouldStop: () => false,
  });

  assert.equal(starter.calls.length, 3, "中间那个失败之后，后面的仍要跑");
  assert.deepEqual(run.results, {
    [rows[0].archivePath]: "succeeded",
    [rows[1].archivePath]: "failed",
    [rows[2].archivePath]: "succeeded",
  });
});

test("a task that cannot be started counts as failed and the batch continues", { timeout: 5_000 }, async () => {
  const rows = dropRowsFromPreviews([preview("a.zip"), preview("b.zip")]);
  const watcher = new ModImportTaskWatcher();
  const calls = [];

  const run = await runDropImportBatch(rows, {
    watcher,
    startImport: async (path) => {
      calls.push(path);
      if (calls.length === 1) throw new Error("storage frozen");
      const task = { kind: "mod_import", status: "queued", taskId: "t2" };
      queueMicrotask(() => watcher.handleProgress(progress(task.taskId, "completed")));
      return task;
    },
    onRunChange: () => {},
    shouldStop: () => false,
  });

  assert.equal(calls.length, 2);
  assert.equal(run.results[rows[0].archivePath], "failed");
  assert.equal(run.results[rows[1].archivePath], "succeeded");
});

test("a start that answers with an unexpected state counts as failed, not as a hang", { timeout: 5_000 }, async () => {
  // 后端若返回非 queued 的状态，就没有任务会发进度事件；不当场判失败的话，
  // 整批会永远停在「正在导入」。
  const rows = dropRowsFromPreviews([preview("a.zip")]);
  const run = await runDropImportBatch(rows, {
    watcher: new ModImportTaskWatcher(),
    startImport: async () => ({ kind: "mod_import", status: "rejected", taskId: "t1" }),
    onRunChange: () => {},
    shouldStop: () => false,
  });
  assert.equal(run.results[rows[0].archivePath], "failed");
  assert.equal(run.currentPath, null);
});

test("stopping mid-batch does not start anything further", { timeout: 5_000 }, async () => {
  const rows = dropRowsFromPreviews([preview("a.zip"), preview("b.zip"), preview("c.zip")]);
  const watcher = new ModImportTaskWatcher();
  const starter = fakeStarter(["t1", "t2", "t3"]);
  let stop = false;

  const run = await runDropImportBatch(rows, {
    watcher,
    startImport: async (path) => {
      const task = await starter.start(path);
      queueMicrotask(() => watcher.handleProgress(progress(task.taskId, "completed")));
      return task;
    },
    onRunChange: () => {
      stop = true;
    },
    shouldStop: () => stop,
  });

  assert.deepEqual(starter.calls, [rows[0].archivePath], "按下停止之后不再起新的");
  assert.equal(run.currentPath, null, "停止之后循环必须结束，不能挂住");
  assert.equal(run.total, 3, "没跑的那两个不记成失败，总数照实保留");
  assert.equal(Object.keys(run.results).length, 1);
});
