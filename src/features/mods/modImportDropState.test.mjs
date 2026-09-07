import assert from "node:assert/strict";
import test from "node:test";

import {
  canStartDropImport,
  dedupeDroppedPaths,
  dropRowsFromPreviews,
  dropSelectAllState,
  getDropRowBlockedMessage,
  importableDropRowCount,
  selectedDropRows,
  setAllDropRowsSelected,
  toggleDropRow,
} from "./modImportDropState.ts";

const preview = (fileName, errorCode = null) => ({
  archivePath: `C:\\downloads\\${fileName}`,
  fileName,
  errorCode,
});

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
    const blocked = getDropRowBlockedMessage(rows[0], copy);
    assert.equal(typeof blocked, "string");
    assert.ok(blocked.trim().length > 0, `${locale}: 被挡住的行必须有非空提示`);
    assert.equal(blocked, copy.errors.archiveEncrypted, `${locale}: 必须复用同一句文案`);
    assert.equal(getDropRowBlockedMessage(rows[1], copy), null, "可导入的行没有提示语");
  }
});

test("duplicate paths in one drop collapse to a single row", () => {
  // 同一个文件起两个导入任务的话，第二个必然失败，玩家看到一条莫名其妙的失败。
  assert.deepEqual(dedupeDroppedPaths(["a.zip", "b.zip", "a.zip", ""]), ["a.zip", "b.zip"]);
});
