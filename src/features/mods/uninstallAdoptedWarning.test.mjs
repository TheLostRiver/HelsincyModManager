// #286 adopt 收尾①：卸载确认对接管条目的提示。
//
// 接管条目没有 backup_ref，卸载只删除。三语文案与接线形状都在这里守着；
// 每条用例都跑过控制组：把实现退回去，确认它会变红。

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { externalStateCopy } from "./externalStateCopy.ts";
import { modLifecycleCopy } from "./modLifecycleCopy.ts";

const currentDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(currentDirectory, "../../..");
const readSource = (name) => readFileSync(join(currentDirectory, name), "utf8");
const readRepo = (relative) => readFileSync(join(repositoryRoot, relative), "utf8");

const LOCALES = ["zh_cn", "en", "ja"];

// 「先用 HMM 重装再卸载就能还原原版」是错的：重装为同路径条目沿用旧条目的 backup_ref
// （接管条目为空 → 仍为空），单修订版 MOD 的重装更会被 candidate_already_installed 挡住。
// 两处接管相关文案都不得再给这条建议。
const REINSTALL_ADVICE = {
  zh_cn: /重新安装|重装/,
  en: /reinstall/i,
  ja: /再インストール/,
};

test("三语卸载提示：织入接管数量、有指标标签，且不再建议「重装后卸载」", () => {
  for (const locale of LOCALES) {
    const lifecycle = modLifecycleCopy[locale];
    const warning = lifecycle.uninstallDialog.adoptedWarning(3);
    assert.match(warning, /3/, `${locale}.adoptedWarning 必须织入数量`);
    assert.doesNotMatch(warning, REINSTALL_ADVICE[locale], `${locale}.adoptedWarning 仍在建议重装`);
    assert.ok(lifecycle.planSheet.metricAdoptedFiles.length > 0, `${locale}.metricAdoptedFiles 为空`);

    const adoptConfirm = externalStateCopy[locale].adopt.confirm.uninstallWarning;
    assert.doesNotMatch(adoptConfirm, REINSTALL_ADVICE[locale], `${locale} 接管确认仍在建议重装`);
    assert.match(adoptConfirm, /Steam/, `${locale} 接管确认应保留 Steam 校验这条唯一可行的还原路径`);
  }
});

const feedbackSource = readSource("ModLifecycleFeedback.tsx");
const pageSource = readSource("ModLibraryPage.tsx");
const loadStateSource = readSource("modLibraryLoadState.ts");

test("卸载确认弹窗：只有接管数 > 0 才渲染提示与「接管文件」指标，缺席不当 0 以外的任何值", () => {
  assert.match(feedbackSource, /adoptedFileCount\?: number;/);
  assert.match(feedbackSource, /const adoptedFileCount = state\.adoptedFileCount \?\? 0;/);
  assert.match(
    feedbackSource,
    /adoptedFileCount > 0\s*\?\s*\[\{ label: lifecycle\.planSheet\.metricAdoptedFiles, value: adoptedFileCount \}\]/,
  );
  assert.match(
    feedbackSource,
    /\{adoptedFileCount > 0 \? \(\s*<p className="mod-lifecycle-feedback__status is-warning">\s*\{uninstallCopy\.adoptedWarning\(adoptedFileCount\)\}/,
  );
});

test("库页面：接管数随耐久摘要进入确认态，并纳入「后端摘要漂移」阻断比对", () => {
  assert.match(pageSource, /adoptedFileCount: item\.installSummary\.adoptedFileCount,/);
  assert.match(
    pageSource,
    /currentSummary\.backupCount === uninstallConfirmation\.backupCount\s*&& currentSummary\.adoptedFileCount === uninstallConfirmation\.adoptedFileCount/,
  );
  // 两条耐久摘要路径都透传；投影派生的 installSummary 不携带，由后端省略键表达。
  assert.match(loadStateSource, /adoptedFileCount: summary\.adoptedFileCount,/);
  assert.match(
    loadStateSource,
    /\.\.\.\(summary\.adoptedFileCount === undefined \? \{\} : \{ adoptedFileCount: summary\.adoptedFileCount \}\)/,
  );
});

test("契约：两条摘要 DTO 都登记了 adoptedFileCount，且不再写「尚未进入任何 DTO」", () => {
  const contract = readRepo("docs/FRONTEND_BACKEND_CONTRACT.md");
  const manifestDto = contract.match(/type InstallManifestStatusSummaryDto = \{[\s\S]*?\};/)?.[0] ?? "";
  const recoveryDto = contract.match(/type InstallRecoverySummaryDto = \{[\s\S]*?\};/)?.[0] ?? "";
  assert.match(manifestDto, /adoptedFileCount\?: number;/);
  assert.match(manifestDto, /installedRevisionId: string \| null;/);
  assert.match(recoveryDto, /adoptedFileCount: number;/);
  assert.doesNotMatch(contract, /尚未进入任何 DTO/);
});
