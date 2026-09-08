import assert from "node:assert/strict";
import test from "node:test";

import {
  canStartDropImport,
  dedupeDroppedPaths,
  dropRowsFromPreviews,
  dropSelectAllState,
  getDropRowNote,
  isDropRowSelectable,
  advanceDropImportRun,
  dropImportRunSummary,
  MAX_DROPPED_ARCHIVES,
  importableDropRowCount,
  selectedDropRows,
  startDropImportRun,
  stopDropImportRun,
  setAllDropRowsSelected,
  toggleDropRow,
} from "./modImportDropState.ts";

const preview = (fileName, errorCode = null, sizeBytes = 1024, warningCode = null) => ({
  archivePath: `C:\\downloads\\${fileName}`,
  fileName,
  sizeBytes,
  errorCode,
  warningCode,
});

/** 内容层警示档：读得了，但看起来装不出东西。 */
const warned = (fileName) =>
  preview(fileName, null, 1024, "mod_import_archive_no_game_content");

test("importable rows default to selected, blocked rows do not", () => {
  const rows = dropRowsFromPreviews([
    preview("good.zip"),
    preview("thing.exe", "mod_import_not_an_archive"),
  ]);

  assert.equal(rows[0].status, "importable");
  assert.equal(rows[0].selected, true);
  assert.equal(rows[0].messageKind, null);

  assert.equal(rows[1].status, "blocked");
  assert.equal(rows[1].selected, false);
  assert.equal(rows[1].messageKind, "not-an-archive");
});

test("every container tier maps to its own message kind", () => {
  // 复用导入失败的档位，不另造词汇——加密与分卷是 T21-C/D 新增的两档。
  const rows = dropRowsFromPreviews([
    preview("a.rar", "mod_import_archive_encrypted"),
    preview("b.rar", "mod_import_archive_multi_volume"),
    preview("c.tar", "mod_import_unsupported_archive_format"),
    preview("d.exe", "mod_import_not_an_archive"),
    preview("e.zip", "mod_import_prepare_failed"),
  ]);
  assert.deepEqual(
    rows.map((row) => row.messageKind),
    [
      "archive-encrypted",
      "archive-multi-volume",
      "unsupported-archive-format",
      "not-an-archive",
      "retry-hint",
    ],
  );
});

test("blocked rows cannot be checked — they are hard facts, not defaults", () => {
  const rows = dropRowsFromPreviews([preview("thing.exe", "mod_import_not_an_archive")]);
  const toggled = toggleDropRow(rows, rows[0].archivePath);
  assert.equal(toggled[0].selected, false, "读不了的文件不该能被勾上");

  const all = setAllDropRowsSelected(rows, true);
  assert.equal(all[0].selected, false, "全选也不该把读不了的文件勾上");
});

test("toggling an importable row flips only that row", () => {
  const rows = dropRowsFromPreviews([preview("a.zip"), preview("b.zip")]);
  const toggled = toggleDropRow(rows, rows[0].archivePath);
  assert.equal(toggled[0].selected, false);
  assert.equal(toggled[1].selected, true);
});

test("select-all state is none when nothing is importable", () => {
  // 否则整批都读不了的拖拽会显示成「已全选」，而确认按钮是灰的——自相矛盾。
  const rows = dropRowsFromPreviews([
    preview("a.exe", "mod_import_not_an_archive"),
    preview("b.exe", "mod_import_not_an_archive"),
  ]);
  assert.equal(dropSelectAllState(rows), "none");
  assert.equal(canStartDropImport(rows), false);
  assert.equal(importableDropRowCount(rows), 0);
});

test("select-all state tracks partial and full selection", () => {
  const rows = dropRowsFromPreviews([preview("a.zip"), preview("b.zip")]);
  assert.equal(dropSelectAllState(rows), "all");

  const partial = toggleDropRow(rows, rows[0].archivePath);
  assert.equal(dropSelectAllState(partial), "some");
  assert.equal(canStartDropImport(partial), true);

  const cleared = setAllDropRowsSelected(rows, false);
  assert.equal(dropSelectAllState(cleared), "none");
  assert.equal(canStartDropImport(cleared), false, "一个都没选时不能确认");
});

test("only importable and selected rows are handed to the import", () => {
  const rows = setAllDropRowsSelected(
    dropRowsFromPreviews([
      preview("good.zip"),
      preview("bad.exe", "mod_import_not_an_archive"),
      preview("also-good.rar"),
    ]),
    true,
  );
  assert.deepEqual(
    selectedDropRows(rows).map((row) => row.fileName),
    ["good.zip", "also-good.rar"],
  );
});

test("blocked rows render the same three-language copy as a failed import", async () => {
  const { modImportCopy } = await import("./modImportCopy.ts");
  const rows = dropRowsFromPreviews([
    preview("secret.rar", "mod_import_archive_encrypted"),
    preview("good.zip"),
  ]);

  for (const locale of ["zh_cn", "en", "ja"]) {
    const copy = modImportCopy[locale];
    const blocked = getDropRowNote(rows[0], copy);
    assert.equal(typeof blocked, "string");
    assert.ok(blocked.trim().length > 0, `${locale}: 被挡住的行必须有非空提示`);
    assert.equal(blocked, copy.errors.archiveEncrypted, `${locale}: 必须复用同一句文案`);
    assert.equal(getDropRowNote(rows[1], copy), null, "可导入的行没有提示语");
  }
});

test("duplicate paths in one drop collapse to a single row", () => {
  // 同一个文件起两个导入任务的话，第二个必然失败，玩家看到一条莫名其妙的失败。
  assert.deepEqual(dedupeDroppedPaths(["a.zip", "b.zip", "a.zip", ""]), ["a.zip", "b.zip"]);
});

test("a run walks the selected rows one at a time and records each outcome", () => {
  const rows = dropRowsFromPreviews([
    preview("a.zip"),
    preview("bad.exe", "mod_import_not_an_archive"),
    preview("b.rar"),
  ]);

  let run = startDropImportRun(rows);
  assert.equal(run.total, 2, "被挡住的行不进队列");
  assert.equal(run.currentPath, rows[0].archivePath);
  assert.equal(dropImportRunSummary(run).done, false);

  run = advanceDropImportRun(run, "succeeded");
  assert.equal(run.currentPath, rows[2].archivePath, "串行推进到下一个");

  run = advanceDropImportRun(run, "failed");
  const summary = dropImportRunSummary(run);
  assert.deepEqual(
    { succeeded: summary.succeeded, failed: summary.failed, done: summary.done },
    { succeeded: 1, failed: 1, done: true },
    "部分成功要如实计数，不能一个失败就整批算失败",
  );
});

test("an empty selection is not reported as a finished run", () => {
  // done 若写成 finished === total，一个都没选时会立刻算「跑完」，而根本没开始。
  const rows = setAllDropRowsSelected(dropRowsFromPreviews([preview("a.zip")]), false);
  const run = startDropImportRun(rows);
  assert.equal(run.total, 0);
  assert.equal(dropImportRunSummary(run).done, true, "空队列本来就没有正在跑的");
  assert.equal(run.currentPath, null);
});

test("stopping a run cancels everything that has not started yet", () => {
  // 调用点的顺序是「跑完 → advance → 停止」，所以停止那一刻 advance 已经把下一个
  // 提升成了 currentPath。它还没起步，停止必须把它也放掉。
  const rows = dropRowsFromPreviews([preview("a.zip"), preview("b.zip"), preview("c.zip")]);
  let run = startDropImportRun(rows);
  run = advanceDropImportRun(run, "succeeded");
  assert.equal(run.currentPath, rows[1].archivePath, "advance 已经提升了下一个");

  run = stopDropImportRun(run);
  assert.equal(run.currentPath, null, "被提升但没起步的那个也要放掉");

  const summary = dropImportRunSummary(run);
  assert.equal(summary.done, true, "停止之后循环必须能结束");
  assert.equal(summary.total, 3, "总数仍是玩家当初选的数量");
  assert.deepEqual(
    { succeeded: summary.succeeded, failed: summary.failed, finished: summary.finished },
    { succeeded: 1, failed: 0, finished: 1 },
    "没跑的既不算成功也不算失败",
  );
});

test("大小读不到不改判可导入性", () => {
  // 判据只有一条：能不能打开归档。大小只是给玩家核对用的旁证。
  const rows = dropRowsFromPreviews([preview("a.zip", null, null)]);
  assert.equal(rows[0].status, "importable");
  assert.equal(rows[0].sizeBytes, null);
});

test("一次拖太多整批拒绝，不静默截断", () => {
  // 截断等于悄悄丢掉玩家拖进来的东西，而他多半不会去数清单有几行。
  assert.equal(typeof MAX_DROPPED_ARCHIVES, "number");
  assert.ok(MAX_DROPPED_ARCHIVES > 0);
  const many = Array.from({ length: MAX_DROPPED_ARCHIVES + 1 }, (_, i) => `C:\\d\\${i}.zip`);
  assert.equal(
    dedupeDroppedPaths(many).length,
    MAX_DROPPED_ARCHIVES + 1,
    "去重不负责设上限——上限是调用方的决定，且必须让玩家看见数量",
  );
});

test("内容层警示默认不勾选，但**必须**能勾回来", () => {
  // 这是这一整档存在的理由：包级否决是错的，我们的判定会错，而错的代价是
  // 玩家眼睁睁看着一个好包装不进来（#350 / #354 那一整轮的教训）。
  const rows = dropRowsFromPreviews([warned("mystery.zip")]);
  assert.equal(rows[0].status, "warned");
  assert.equal(rows[0].selected, false, "默认不勾选");
  assert.equal(isDropRowSelectable(rows[0]), true, "警示档必须可勾");

  const toggled = toggleDropRow(rows, rows[0].archivePath);
  assert.equal(toggled[0].selected, true, "玩家必须能覆盖我们的判断");
  assert.deepEqual(
    selectedDropRows(toggled).map((row) => row.fileName),
    ["mystery.zip"],
    "勾回来之后必须真的进导入队列",
  );
});

test("警示档与读不了的档不能混同", () => {
  const rows = dropRowsFromPreviews([
    warned("mystery.zip"),
    preview("secret.rar", "mod_import_archive_encrypted"),
  ]);
  assert.equal(isDropRowSelectable(rows[0]), true, "警示档可勾");
  assert.equal(isDropRowSelectable(rows[1]), false, "读不了的不可勾");

  const all = setAllDropRowsSelected(rows, true);
  assert.equal(all[0].selected, true, "全选要把警示档勾上");
  assert.equal(all[1].selected, false, "全选不该把读不了的勾上");
});

test("警示档算进「可选」的分母，全选状态才不会自相矛盾", () => {
  const rows = dropRowsFromPreviews([preview("a.zip"), warned("b.zip")]);
  assert.equal(importableDropRowCount(rows), 2);
  assert.equal(dropSelectAllState(rows), "some", "一个勾了一个没勾");
  assert.equal(canStartDropImport(rows), true);

  assert.equal(dropSelectAllState(setAllDropRowsSelected(rows, true)), "all");
  assert.equal(dropSelectAllState(setAllDropRowsSelected(rows, false)), "none");
});

test("警示语必须三语齐全，而且要说清仍然能导入", async () => {
  const { modImportCopy } = await import("./modImportCopy.ts");
  const rows = dropRowsFromPreviews([warned("mystery.zip")]);
  for (const locale of ["zh_cn", "en", "ja"]) {
    const copy = modImportCopy[locale];
    const note = getDropRowNote(rows[0], copy);
    assert.equal(note, copy.drop.warnNoGameContent, `${locale}: 必须走警示文案`);
    assert.ok(note.trim().length > 0, `${locale}: 警示语不能为空`);
    // 不能复用「读不了」那套词汇——那会让玩家以为这行根本导不了。
    assert.notEqual(note, copy.errors.notAnArchive, `${locale}: 不能与读不了的档同一句`);
  }
});

test("认不出的警示码当成没有警示，而不是默认取消勾选", () => {
  // 后端将来多发一个我们还不认识的码时，最坏结果应该是「少说一句提示」，
  // 而不是「一整批包突然都默认不勾选了」。
  const rows = dropRowsFromPreviews([preview("a.zip", null, 1024, "mod_import_future_warning")]);
  assert.equal(rows[0].status, "importable");
  assert.equal(rows[0].selected, true);
  assert.equal(rows[0].warningKind, null);
});
