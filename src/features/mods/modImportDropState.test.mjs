import assert from "node:assert/strict";
import test from "node:test";

import {
  canStartDropImport,
  cancelQueuedDropRows,
  clearFinishedDropRows,
  confirmDropSelection,
  dedupeDroppedPaths,
  dropQueueSummary,
  dropRowsFromPreviews,
  dropSelectAllState,
  getDropRowNote,
  isDropRowSelectable,
  markDropRowPhase,
  MAX_DROPPED_ARCHIVES,
  mergeDropRows,
  pendingDropRows,
  selectableDropRowCount,
  selectedDropRows,
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

const pathOf = (fileName) => `C:\\downloads\\${fileName}`;

// ---- 预检结论（status 这条轴）----

test("可导入的行默认勾选，读不了的行不勾也不能勾", () => {
  const rows = dropRowsFromPreviews([
    preview("good.zip"),
    preview("thing.exe", "mod_import_not_an_archive"),
  ]);

  assert.equal(rows[0].status, "importable");
  assert.equal(rows[0].selected, true);
  assert.equal(rows[0].phase, "pending");

  assert.equal(rows[1].status, "blocked");
  assert.equal(rows[1].selected, false);
  assert.equal(isDropRowSelectable(rows[1]), false);
  assert.equal(rows[1].messageKind, "not-an-archive");
});

test("每个容器层档位映射到自己的档位词汇", () => {
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

test("内容层警示默认不勾选，但**必须**能勾回来", () => {
  // 这一整档存在的理由：包级否决是错的，我们的判定会错，而错的代价是玩家眼睁睁
  // 看着一个好包装不进来（#350 / #354 那一整轮的教训）。
  const rows = dropRowsFromPreviews([warned("mystery.zip")]);
  assert.equal(rows[0].status, "warned");
  assert.equal(rows[0].selected, false, "默认不勾选");
  assert.equal(isDropRowSelectable(rows[0]), true, "警示档必须可勾");

  const toggled = toggleDropRow(rows, rows[0].archivePath);
  assert.equal(toggled[0].selected, true, "玩家必须能覆盖我们的判断");
  assert.deepEqual(selectedDropRows(toggled).map((row) => row.fileName), ["mystery.zip"]);
});

test("全选把警示档勾上，但不碰读不了的行", () => {
  const rows = dropRowsFromPreviews([
    warned("mystery.zip"),
    preview("secret.rar", "mod_import_archive_encrypted"),
  ]);
  const all = setAllDropRowsSelected(rows, true);
  assert.equal(all[0].selected, true, "全选要把警示档勾上");
  assert.equal(all[1].selected, false, "全选不该把读不了的勾上");
});

test("一个可勾的都没有时全选状态是 none，不是 all", () => {
  // 否则整批都读不了的拖拽会显示成「已全选」，而确认按钮是灰的——自相矛盾。
  const rows = dropRowsFromPreviews([
    preview("a.exe", "mod_import_not_an_archive"),
    preview("b.exe", "mod_import_not_an_archive"),
  ]);
  assert.equal(dropSelectAllState(rows), "none");
  assert.equal(canStartDropImport(rows), false);
  assert.equal(selectableDropRowCount(rows), 0);
});

test("认不出的警示码当成没有警示，而不是默认取消勾选", () => {
  // 后端将来多发一个我们还不认识的码时，最坏结果应该是「少说一句提示」，
  // 而不是「一整批包突然都默认不勾选了」。
  const rows = dropRowsFromPreviews([preview("a.zip", null, 1024, "mod_import_future_warning")]);
  assert.equal(rows[0].status, "importable");
  assert.equal(rows[0].selected, true);
  assert.equal(rows[0].warningKind, null);
});

test("大小读不到不改判可导入性", () => {
  const rows = dropRowsFromPreviews([preview("a.zip", null, null)]);
  assert.equal(rows[0].status, "importable");
  assert.equal(rows[0].sizeBytes, null);
});

test("提示语三语齐全；警示语不能与「读不了」同一句", async () => {
  const { modImportCopy } = await import("./modImportCopy.ts");
  const rows = dropRowsFromPreviews([
    preview("secret.rar", "mod_import_archive_encrypted"),
    warned("mystery.zip"),
    preview("good.zip"),
  ]);
  for (const locale of ["zh_cn", "en", "ja"]) {
    const copy = modImportCopy[locale];
    assert.equal(getDropRowNote(rows[0], copy), copy.errors.archiveEncrypted, `${locale}: 复用同一句`);
    const warn = getDropRowNote(rows[1], copy);
    assert.equal(warn, copy.drop.warnNoGameContent, `${locale}: 警示走自己的文案`);
    assert.notEqual(warn, copy.errors.notAnArchive, `${locale}: 不能与读不了的档同一句`);
    assert.equal(getDropRowNote(rows[2], copy), null, "可导入的行没有提示语");
  }
});

// ---- 执行生命周期（phase 这条轴）----

test("确认之后：选中的行变 queued 并给出入队路径，其余不动", () => {
  const rows = setAllDropRowsSelected(
    dropRowsFromPreviews([
      preview("a.zip"),
      preview("bad.exe", "mod_import_not_an_archive"),
      preview("b.zip"),
    ]),
    true,
  );

  const { rows: next, queued } = confirmDropSelection(rows);
  assert.deepEqual(queued, [pathOf("a.zip"), pathOf("b.zip")], "读不了的不入队");
  assert.deepEqual(next.map((row) => row.phase), ["queued", "pending", "queued"]);
  assert.equal(next[0].selected, false, "已提交的行不再顶着勾");
  assert.equal(isDropRowSelectable(next[0]), false, "已提交的行不归玩家管了");
  assert.equal(canStartDropImport(next), false, "没有待确认的了，确认按钮该灰");
});

test("已提交的行不响应勾选与全选", () => {
  const { rows } = confirmDropSelection(dropRowsFromPreviews([preview("a.zip")]));
  assert.equal(toggleDropRow(rows, rows[0].archivePath)[0].selected, false);
  assert.equal(setAllDropRowsSelected(rows, true)[0].selected, false);
});

test("停止排队把还没起步的退回 pending，不碰正在跑的那个", () => {
  // 退回而不是记成失败：它根本没跑过，记成失败是撒谎；退回之后玩家还能再确认一次。
  let rows = confirmDropSelection(
    setAllDropRowsSelected(
      dropRowsFromPreviews([preview("a.zip"), preview("b.zip"), preview("c.zip")]),
      true,
    ),
  ).rows;
  rows = markDropRowPhase(rows, pathOf("a.zip"), "running");

  const stopped = cancelQueuedDropRows(rows);
  assert.deepEqual(stopped.map((row) => row.phase), ["running", "pending", "pending"]);
  assert.equal(stopped[1].selected, true, "退回之后要保持勾选，玩家能直接再确认");
  assert.equal(canStartDropImport(stopped), true);
});

test("摘要如实分开计数，不把部分成功说成整批失败", () => {
  let rows = confirmDropSelection(
    setAllDropRowsSelected(
      dropRowsFromPreviews([preview("a.zip"), preview("b.zip"), preview("c.zip")]),
      true,
    ),
  ).rows;
  rows = markDropRowPhase(rows, pathOf("a.zip"), "succeeded");
  rows = markDropRowPhase(rows, pathOf("b.zip"), "failed");
  rows = markDropRowPhase(rows, pathOf("c.zip"), "running");

  const summary = dropQueueSummary(rows);
  assert.deepEqual(
    {
      succeeded: summary.succeeded,
      failed: summary.failed,
      running: summary.running,
      submitted: summary.submitted,
      active: summary.active,
    },
    { succeeded: 1, failed: 1, running: 1, submitted: 3, active: true },
  );

  const done = markDropRowPhase(rows, pathOf("c.zip"), "succeeded");
  assert.equal(dropQueueSummary(done).active, false, "全跑完就不再 active");
});

test("一个都没提交时不算 active", () => {
  // active 若写成「有行就算」，光拖进来还没确认也会显示成正在导入。
  const rows = dropRowsFromPreviews([preview("a.zip")]);
  assert.equal(dropQueueSummary(rows).active, false);
  assert.equal(dropQueueSummary(rows).submitted, 0);
});

test("清除已完成只清跑完的，留下待确认与在跑的", () => {
  let rows = confirmDropSelection(
    setAllDropRowsSelected(dropRowsFromPreviews([preview("a.zip"), preview("b.zip")]), true),
  ).rows;
  rows = markDropRowPhase(rows, pathOf("a.zip"), "succeeded");
  rows = markDropRowPhase(rows, pathOf("b.zip"), "running");
  rows = mergeDropRows(rows, [preview("c.zip")]);

  const cleared = clearFinishedDropRows(rows);
  assert.deepEqual(cleared.map((row) => row.fileName), ["b.zip", "c.zip"]);
});

// ---- 追加（清单是长活的）----

test("导入进行中再拖一批：新的并进来，已在队列里的跳过", () => {
  // 重复入队会对同一个文件起两个导入任务，第二个必然失败，玩家看到一条莫名其妙的失败。
  let rows = confirmDropSelection(dropRowsFromPreviews([preview("a.zip")])).rows;
  rows = markDropRowPhase(rows, pathOf("a.zip"), "running");

  const merged = mergeDropRows(rows, [preview("a.zip"), preview("b.zip")]);
  assert.deepEqual(merged.map((row) => row.fileName), ["a.zip", "b.zip"]);
  assert.equal(merged[0].phase, "running", "正在跑的那个不能被重置");
  assert.equal(merged[1].phase, "pending");
});

test("重拖一个已经跑完的包会重置成 pending，可以重试", () => {
  // 清单是长活的。不重置的话，一个失败过的包在关掉浮层之前永远没法再试。
  let rows = confirmDropSelection(dropRowsFromPreviews([preview("a.zip")])).rows;
  rows = markDropRowPhase(rows, pathOf("a.zip"), "failed");

  const merged = mergeDropRows(rows, [preview("a.zip")]);
  assert.equal(merged.length, 1, "不该多出一行");
  assert.equal(merged[0].phase, "pending");
  assert.equal(merged[0].selected, true, "重置之后默认勾上，玩家确认即可重试");
  assert.equal(pendingDropRows(merged).length, 1);
});

test("追加保持原有顺序，新行接在后面", () => {
  const rows = dropRowsFromPreviews([preview("a.zip"), preview("b.zip")]);
  const merged = mergeDropRows(rows, [preview("c.zip"), preview("a.zip")]);
  assert.deepEqual(merged.map((row) => row.fileName), ["a.zip", "b.zip", "c.zip"]);
});

// ---- 入口守卫 ----

test("同一次拖拽里的重复路径只留一条", () => {
  assert.deepEqual(dedupeDroppedPaths(["a.zip", "b.zip", "a.zip", ""]), ["a.zip", "b.zip"]);
});

test("去重不负责设上限——上限是调用方的决定，且必须让玩家看见数量", () => {
  assert.equal(typeof MAX_DROPPED_ARCHIVES, "number");
  assert.ok(MAX_DROPPED_ARCHIVES > 0);
  const many = Array.from({ length: MAX_DROPPED_ARCHIVES + 1 }, (_, i) => `C:\\d\\${i}.zip`);
  assert.equal(dedupeDroppedPaths(many).length, MAX_DROPPED_ARCHIVES + 1);
});
