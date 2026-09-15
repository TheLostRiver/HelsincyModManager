import assert from "node:assert/strict";
import test from "node:test";
import {
  canStartDropImport, dedupeDroppedPaths, dropQueueSummary, dropRowsFromPreviews, dropSelectAllState,
  getDropRowNote, isDropRowSelectable, MAX_DROPPED_ARCHIVES, selectableDropRowCount,
  selectedDropRows, setAllDropRowsSelected,
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

  const toggled = setAllDropRowsSelected(rows, true);
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

test("执行结果分别统计，跳过项不进入提交总数", () => {
  const phases = ["pending", "queued", "running", "succeeded", "failed", "cancelled", "skipped"];
  const rows = phases.map((phase, i) => ({ ...dropRowsFromPreviews([preview(i + ".zip")])[0], phase }));
  assert.deepEqual(dropQueueSummary(rows), {
    pending: 1, queued: 1, running: 1, succeeded: 1, failed: 1, cancelled: 1, skipped: 1, submitted: 5, active: true,
  });
});

test("全选不改变已提交或已结束的条目", () => {
  const phases = ["queued", "running", "succeeded", "failed", "cancelled", "skipped"];
  const rows = phases.map((phase, i) => ({ ...dropRowsFromPreviews([preview(i + ".zip")])[0], selected: false, phase }));
  assert.deepEqual(setAllDropRowsSelected(rows, true), rows);
  assert.equal(canStartDropImport(rows), false);
});

test("取消排队保留可理解的三语原因", async () => {
  const { modImportCopy } = await import("./modImportCopy.ts");
  const row = { ...dropRowsFromPreviews([preview("a.zip")])[0], phase: "cancelled", selected: false };
  for (const locale of ["zh_cn", "en", "ja"]) {
    assert.equal(getDropRowNote(row, modImportCopy[locale]), modImportCopy[locale].drop.cancelledBeforeStart);
  }
});

test("去重跳过空项但不截断用户输入", () => {
  const a = pathOf("a.zip");
  const b = pathOf("b.zip");
  assert.deepEqual(dedupeDroppedPaths([a, "", b, a]), [a, b]);
  const paths = Array.from({ length: MAX_DROPPED_ARCHIVES + 1 }, (_, i) => pathOf(i + ".zip"));
  assert.equal(dedupeDroppedPaths(paths).length, MAX_DROPPED_ARCHIVES + 1);
});
