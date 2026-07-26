import assert from "node:assert/strict";
import { performance } from "node:perf_hooks";
import { test } from "node:test";

import {
  EXTERNAL_IMPORT_RESULT_10000_VALIDATION_BUDGET_MS,
  appendExternalImportResults,
  getExternalImportBatchStatusLabel,
  getExternalImportResultErrorMessage,
  isExternalImportBatchResultPageForBatch,
  summarizeExternalImportResults,
  toExternalImportResultViewModel,
} from "./externalImportResultModel.ts";

function item(overrides = {}) {
  return {
    candidateId: "candidate-a",
    status: "imported",
    reasonCode: null,
    importedModId: "mod-a",
    retryable: false,
    ...overrides,
  };
}

function resultPage(overrides = {}) {
  return {
    batch: {
      batchId: "batch-a",
      adapterId: "hunting_box_directory_v1",
      scanStatus: "completed",
      importStatus: "completed_with_errors",
    },
    results: [item()],
    totalCount: 1,
    nextCursor: null,
    ...overrides,
  };
}

test("result page accepts only the exact redacted terminal contract", () => {
  assert.equal(isExternalImportBatchResultPageForBatch(resultPage(), "batch-a"), true);
  assert.equal(
    isExternalImportBatchResultPageForBatch(
      resultPage({ batch: { ...resultPage().batch, importStatus: "running" } }),
      "batch-a",
    ),
    false,
  );
  assert.equal(
    isExternalImportBatchResultPageForBatch(
      resultPage({ nextCursor: "next/page" }),
      "batch-a",
    ),
    false,
  );
  assert.equal(
    isExternalImportBatchResultPageForBatch(
      resultPage({ rawPath: "C:\\private\\source" }),
      "batch-a",
    ),
    false,
  );
  assert.equal(
    isExternalImportBatchResultPageForBatch(
      resultPage({ results: [item({ sourceFingerprint: "private" })] }),
      "batch-a",
    ),
    false,
  );
});

test("result validation rejects unsafe identities, unknown status/reason, duplicates, and oversized pages", () => {
  assert.equal(
    isExternalImportBatchResultPageForBatch(
      resultPage({ results: [item({ candidateId: "candidate/path" })] }),
      "batch-a",
    ),
    false,
  );
  assert.equal(
    isExternalImportBatchResultPageForBatch(
      resultPage({ results: [item({ status: "future_status" })] }),
      "batch-a",
    ),
    false,
  );
  assert.equal(
    isExternalImportBatchResultPageForBatch(
      resultPage({ results: [item({ reasonCode: "future_reason" })] }),
      "batch-a",
    ),
    false,
  );
  assert.equal(
    isExternalImportBatchResultPageForBatch(
      resultPage({ totalCount: 2, nextCursor: "02" }),
      "batch-a",
    ),
    false,
  );
  assert.equal(
    isExternalImportBatchResultPageForBatch(
      resultPage({ nextCursor: "9007199254740992" }),
      "batch-a",
    ),
    false,
  );
  assert.equal(
    isExternalImportBatchResultPageForBatch(
      resultPage({ nextCursor: "1" }),
      "batch-a",
    ),
    false,
  );
  assert.equal(
    isExternalImportBatchResultPageForBatch(
      resultPage({
        results: [item(), item({ importedModId: "mod-b" })],
        totalCount: 2,
      }),
      "batch-a",
    ),
    false,
  );

  const oversized = Array.from({ length: 101 }, (_, index) =>
    item({ candidateId: `candidate-${index}`, importedModId: `mod-${index}` }),
  );
  assert.equal(
    isExternalImportBatchResultPageForBatch(
      resultPage({ results: oversized, totalCount: oversized.length }),
      "batch-a",
    ),
    false,
  );
});

test("result presentation and summary distinguish partial success without event-count inference", () => {
  const results = [
    item(),
    item({
      candidateId: "candidate-b",
      status: "already_imported",
      reasonCode: "already_imported",
      importedModId: "mod-existing",
    }),
    item({
      candidateId: "candidate-c",
      status: "failed",
      importedModId: null,
      retryable: true,
    }),
    item({
      candidateId: "candidate-d",
      status: "blocked",
      reasonCode: "source_changed",
      importedModId: null,
    }),
  ];

  const summary = summarizeExternalImportResults(results);
  assert.deepEqual(summary, {
    imported: 1,
    alreadyImported: 1,
    skipped: 0,
    blocked: 1,
    failed: 1,
    cancelled: 0,
    retryable: 1,
  });
  assert.equal(getExternalImportBatchStatusLabel("completed_with_errors"), "部分完成");

  const failed = toExternalImportResultViewModel(results[2]);
  assert.equal(failed.statusLabel, "导入失败");
  assert.equal(failed.reasonLabel, "可重试");
  assert.equal(failed.retryable, true);

  const blocked = toExternalImportResultViewModel(results[3]);
  assert.equal(blocked.reasonLabel, "来源已变化");
  assert.equal(blocked.statusTone, "danger");
});

test("result pagination deduplicates candidate ids and unknown UI errors remain stable", () => {
  const existing = [toExternalImportResultViewModel(item())];
  const merged = appendExternalImportResults(existing, [
    item(),
    item({ candidateId: "candidate-b", importedModId: null, status: "skipped" }),
  ]);

  assert.deepEqual(
    merged.map((result) => result.candidateId),
    ["candidate-a", "candidate-b"],
  );
  assert.equal(
    getExternalImportResultErrorMessage("C:\\private\\raw-error"),
    "无法读取批量导入结果，请稍后重试",
  );
});

test("10,000 artificial redacted results keep fixed page validation within budget", (testContext) => {
  const pages = Array.from({ length: 100 }, (_, pageIndex) => {
    const results = Array.from({ length: 100 }, (_, itemIndex) => {
      const index = pageIndex * 100 + itemIndex;
      return item({
        candidateId: `candidate-${index}`,
        importedModId: `mod-${index}`,
        status: index % 7 === 0 ? "failed" : "imported",
        retryable: index % 7 === 0,
      });
    });
    return resultPage({
      results,
      totalCount: 10_000,
      nextCursor: pageIndex === 99 ? null : String((pageIndex + 1) * 100),
    });
  });

  const validateAllPages = () => {
    for (const page of pages) {
      assert.equal(isExternalImportBatchResultPageForBatch(page, "batch-a"), true);
    }
  };
  for (let index = 0; index < 5; index += 1) {
    validateAllPages();
  }

  const samples = Array.from({ length: 40 }, () => {
    const startedAt = performance.now();
    validateAllPages();
    return performance.now() - startedAt;
  }).sort((left, right) => left - right);
  const p95Millis = samples[37];
  testContext.diagnostic(
    `10,000-result page validation p95=${p95Millis.toFixed(3)} ms ` +
      `(budget=${EXTERNAL_IMPORT_RESULT_10000_VALIDATION_BUDGET_MS} ms)`,
  );

  assert.ok(
    p95Millis <= EXTERNAL_IMPORT_RESULT_10000_VALIDATION_BUDGET_MS,
    `10,000-result page validation p95 ${p95Millis.toFixed(3)} ms exceeded ` +
      `${EXTERNAL_IMPORT_RESULT_10000_VALIDATION_BUDGET_MS} ms`,
  );
});
