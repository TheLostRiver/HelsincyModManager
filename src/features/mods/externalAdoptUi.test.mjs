// #286 adopt 前端：可用性投影（纯逻辑）、三语文案与稳定码完备性、以及接线的源码形状门禁。
//
// 每条用例都跑过控制组：把实现退回去，确认它会变红。

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import {
  externalAdoptCounts,
  projectExternalAdoptAvailability,
} from "./externalAdoptView.ts";
import {
  externalAdoptErrorMessage,
  externalStateCopy,
} from "./externalStateCopy.ts";

const currentDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(currentDirectory, "../../..");
const readSource = (name) => readFileSync(join(currentDirectory, name), "utf8");
const readRepo = (relative) => readFileSync(join(repositoryRoot, relative), "utf8");

const LOCALES = ["zh_cn", "en", "ja"];

function file(targetPath, state, claimedByModId) {
  return claimedByModId === undefined
    ? { targetPath, state }
    : { targetPath, state, claimedByModId };
}

function stateOf(files, { stale = false } = {}) {
  const counts = { matched: 0, missing: 0, changed: 0, unreadable: 0 };
  for (const entry of files) counts[entry.state] += 1;
  return {
    summary: {
      state: "installed",
      matchedFileCount: counts.matched,
      missingFileCount: counts.missing,
      changedFileCount: counts.changed,
      unreadableFileCount: counts.unreadable,
      occupiedBy: [],
      files,
    },
    stale,
    lastError: null,
  };
}

// ---- 可用性投影 ----

test("可用性：一致且无主的文件可接管，其余按后端口径分类计数", () => {
  const availability = projectExternalAdoptAvailability(
    stateOf([
      file("nativePC/a.mod3", "matched"),
      file("nativePC/b.mod3", "matched"),
      file("nativePC/c.mod3", "matched", "mod-other"),
      file("nativePC/d.mod3", "changed"),
      file("nativePC/e.mod3", "missing"),
    ]),
  );

  assert.deepEqual(availability, {
    status: "available",
    counts: { claimable: 2, skippedChanged: 1, skippedMissing: 1, skippedClaimed: 1 },
  });
});

test("可用性：被占用但已改动的文件算「已改动」而不是「被占用」——与后端 derive 同口径", () => {
  const counts = externalAdoptCounts(
    stateOf([
      file("nativePC/a.mod3", "matched"),
      file("nativePC/b.mod3", "changed", "mod-other"),
    ]).summary,
  );

  assert.deepEqual(counts, {
    claimable: 1,
    skippedChanged: 1,
    skippedMissing: 0,
    skippedClaimed: 0,
  });
});

test("可用性：前置拒绝按优先级——无记录 → 空集 → 读不到 → 过期 → 无可认领", () => {
  assert.deepEqual(projectExternalAdoptAvailability(null), {
    status: "blocked",
    reason: "no_summary",
  });
  assert.deepEqual(
    projectExternalAdoptAvailability({ summary: null, stale: false, lastError: "x" }),
    { status: "blocked", reason: "no_summary" },
  );
  assert.deepEqual(projectExternalAdoptAvailability(stateOf([])), {
    status: "blocked",
    reason: "unknown",
  });
  // 读不到 + 过期同时成立：读不到优先，因为它还要先解除占用，提示更具体。
  assert.deepEqual(
    projectExternalAdoptAvailability(
      stateOf(
        [file("nativePC/a.mod3", "matched"), file("nativePC/b.mod3", "unreadable")],
        { stale: true },
      ),
    ),
    { status: "blocked", reason: "unreadable" },
  );
  assert.deepEqual(
    projectExternalAdoptAvailability(
      stateOf([file("nativePC/a.mod3", "matched")], { stale: true }),
    ),
    { status: "blocked", reason: "stale" },
  );
  // 全部被占用 / 全部改动或缺失：没有任何可写的条目。
  assert.deepEqual(
    projectExternalAdoptAvailability(
      stateOf([
        file("nativePC/a.mod3", "matched", "mod-other"),
        file("nativePC/b.mod3", "changed"),
        file("nativePC/c.mod3", "missing"),
      ]),
    ),
    { status: "blocked", reason: "nothing_to_adopt" },
  );
});

// ---- 文案 ----

function extractFunctionBody(source, signature) {
  const start = source.indexOf(signature);
  assert.notEqual(start, -1, `missing ${signature}`);
  const bodyStart = source.indexOf("{", start);
  let depth = 0;
  for (let index = bodyStart; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    else if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(bodyStart, index + 1);
    }
  }
  assert.fail(`unbalanced braces for ${signature}`);
}

function extractMappingValues(body) {
  return [...body.matchAll(/=>\s*"([a-z_]+)"/g)].map((match) => match[1]);
}

// 缺 key 时 externalAdoptErrorMessage 会静默回落到 generic，三语 satisfies 也拦不住
// 「码根本没写进联合类型」这种漏——所以直接从后端四个来源取全部稳定码逐个核对。
test("三语错误文案覆盖后端能发出的每一个接管稳定码（含沿用的准入码）", () => {
  const adopter = readRepo("src-tauri/crates/hmm-runtime/src/external_mod_adopt.rs");
  const tasks = readRepo("src-tauri/crates/hmm-runtime/src/external_mod_adopt_tasks.rs");
  const installTask = readRepo("src-tauri/crates/hmm-app/src/install_task.rs");
  const writeAdmission = readRepo("src-tauri/crates/hmm-ports/src/write_admission.rs");

  const codes = [
    ...extractMappingValues(extractFunctionBody(adopter, "pub fn code(&self) -> &'static str")),
    ...extractMappingValues(
      extractFunctionBody(tasks, "pub const fn code(self) -> &'static str"),
    ),
    ...extractMappingValues(
      extractFunctionBody(installTask, "fn failure_phase(&self) -> &'static str"),
    ),
    ...extractMappingValues(
      extractFunctionBody(writeAdmission, "pub const fn code(self) -> &'static str"),
    ),
  ];
  assert.ok(codes.includes("external_mod_adopt_stale") && codes.includes("write_admission_busy"));

  for (const locale of LOCALES) {
    const copy = externalStateCopy[locale];
    const generic = copy.adopt.errors.generic("PROBE");
    for (const code of codes) {
      const message = externalAdoptErrorMessage(code, copy);
      assert.ok(message.length > 0, `${locale}: ${code} 文案为空`);
      assert.ok(
        !message.includes(code) && message !== generic.replace("PROBE", code),
        `${locale}: ${code} 回落到了通用文案——缺专属 key`,
      );
    }
    // 未知码保留原码可见，不吞。
    assert.match(externalAdoptErrorMessage("external_mod_adopt_totally_new", copy), /totally_new/);
  }
});

test("三语接管文案：按钮与确认都织入可接管数量，跳过说明只列非零项", () => {
  for (const locale of LOCALES) {
    const adopt = externalStateCopy[locale].adopt;
    assert.match(adopt.action(7), /7/, `${locale}.action`);
    assert.match(adopt.confirm.confirm(7), /7/, `${locale}.confirm.confirm`);
    assert.match(adopt.confirm.body(7), /7/, `${locale}.confirm.body`);
    assert.match(adopt.completed(7), /7/, `${locale}.completed`);

    const onlyClaimed = adopt.confirm.skipped({
      claimable: 1,
      skippedChanged: 0,
      skippedMissing: 0,
      skippedClaimed: 4,
    });
    assert.match(onlyClaimed, /4/, `${locale}.skipped 必须织入被占用数`);
    assert.ok(!/\b0\b/.test(onlyClaimed), `${locale}.skipped 不得列出为零的分类：${onlyClaimed}`);

    for (const reason of ["unknown", "unreadable", "stale", "nothing_to_adopt"]) {
      assert.ok(adopt.blocked[reason].length > 0, `${locale}.blocked.${reason} 不得为空`);
    }
    assert.ok(adopt.confirm.uninstallWarning.length > 0);
    assert.ok(adopt.completedAuditDegraded.length > 0);
  }
});

// ---- 接线的源码形状 ----

const apiSource = readSource("externalStateApi.ts");
const hookSource = readSource("useExternalModState.ts");
const sectionSource = readSource("ExternalStateSection.tsx");
const confirmSource = readSource("ExternalAdoptConfirmDialog.tsx");
const dialogSource = readSource("ModDetailDialog.tsx");

test("API：接管命令只传 id 与安装同形的 layer 摘要（base / 0），不传任何路径", () => {
  assert.match(apiSource, /invoke<ExternalModAdoptStartedDto>\("start_external_mod_adopt"/);
  assert.match(apiSource, /layerName: "base", layerPriority: 0/);
  assert.doesNotMatch(apiSource, /targetPath|gameRoot|sandbox|archivePath/);
});

test("hook：接管与扫描共用一个监听器，按 kind 分流，互不重叠", () => {
  assert.match(hookSource, /newTaskFlow\("external_state_scan"\)/);
  assert.match(hookSource, /newTaskFlow\("external_mod_adopt"\)/);
  // 一个 listen 订阅，两个 flow 各自接收自己 kind 的终态事件。
  assert.equal((hookSource.match(/void listen<TaskProgressEventDto>/g) ?? []).length, 1);
  assert.match(hookSource, /acceptTerminalEvent\(scanFlowRef\.current, event\.payload, finishScan\)/);
  assert.match(hookSource, /acceptTerminalEvent\(adoptFlowRef\.current, event\.payload, finishAdopt\)/);
  // 互斥：扫描进行中不许接管（接管消费的正是那份记录），反向亦然。
  assert.match(hookSource, /const startAdopt = useCallback\(\(\) => \{\s*if \(isFlowActive\(scanFlowRef\.current\)\)/);
  assert.match(hookSource, /const startScan = useCallback\(\(\) => \{\s*if \(isFlowActive\(adoptFlowRef\.current\)\)/);
  // 重新检查之后，上一次接管的结论（比如 stale）就过期了，不能还挂着。
  assert.match(hookSource, /const startScan = useCallback\([\s\S]*?setAdoptErrorCode\(null\);[\s\S]*?launch\(/);
});

test("hook：接管终态——成功先重查再回调（记录已被后端丢弃）、失败带稳定码并重查、取消不算失败", () => {
  const finishAdopt = extractFunctionBody(hookSource, "const finishAdopt = useCallback(");
  assert.match(finishAdopt, /event\.status === "failed"[\s\S]*?setAdoptErrorCode\(event\.error \?\? "external_mod_adopt_unavailable"\)[\s\S]*?refresh\(\)/);
  assert.match(finishAdopt, /event\.status === "cancelled"[\s\S]*?setAdoptErrorCode\("external_mod_adopt_cancelled"\)/);
  // 成功：refresh 在回调之前，且降级只看 completed 事件上有没有 error。
  const refreshIndex = finishAdopt.lastIndexOf("refresh();");
  const callbackIndex = finishAdopt.indexOf("onAdoptCompletedRef.current?.(");
  assert.ok(refreshIndex !== -1 && callbackIndex !== -1 && refreshIndex < callbackIndex);
  assert.match(finishAdopt, /auditDegraded: event\.error !== null/);
});

test("section：按钮按可用性亮灭、经 alertdialog 二次确认、忙时上报弹窗、完成通知带数量", () => {
  assert.match(sectionSource, /projectExternalAdoptAvailability\(workflow\.state\)/);
  assert.match(
    sectionSource,
    /disabled=\{adoptCounts === null \|\| busy \|\| !workflow\.listenerReady\}/,
  );
  // 点按钮只开确认，确认里才真正启动。
  assert.match(sectionSource, /onClick=\{requestAdopt\}/);
  assert.match(sectionSource, /setConfirmOpen\(true\)/);
  assert.match(sectionSource, /onConfirm=\{confirmAdopt\}/);
  const confirmAdopt = extractFunctionBody(sectionSource, "const confirmAdopt = () =>");
  assert.match(confirmAdopt, /confirmedClaimableRef\.current = adoptCounts\.claimable/);
  assert.match(confirmAdopt, /workflow\.startAdopt\(\)/);
  // 忙态上报 + 卸载时归零，弹窗据此禁止关闭/切页。
  assert.match(sectionSource, /onBusyChangeRef\.current\?\.\(workflow\.adopting\)/);
  assert.match(sectionSource, /useEffect\(\(\) => \(\) => onBusyChangeRef\.current\?\.\(false\), \[\]\)/);
  // 失败文案走接管专属映射；禁用原因有提示行（首扫前除外）。
  assert.match(sectionSource, /externalAdoptErrorMessage\(workflow\.adoptErrorCode, copy\)/);
  assert.match(sectionSource, /availability\.reason !== "no_summary"/);
  assert.match(sectionSource, /copy\.adopt\.blocked\[availability\.reason\]/);
  // 完成后记录已被后端丢弃：区块在卸载前显示完成说明，而不是闪一下「尚未检查过」。
  assert.match(
    sectionSource,
    /setCompletedNotice\(notice\);\s*return onAdoptCompletedRef\.current\?\.\(\{ notice \}\)/,
  );
  assert.match(
    sectionSource,
    /completedNotice !== null \? \(\s*<p className="mod-detail-dialog__external-notice is-occupied" role="status">/,
  );
});

test("确认弹窗：alertdialog、点遮罩不关、初始焦点在取消、正文含卸载后果", () => {
  assert.match(confirmSource, /role="alertdialog"/);
  assert.match(confirmSource, /closeOnBackdrop=\{false\}/);
  assert.match(confirmSource, /initialFocusRef=\{cancelButtonRef\}/);
  assert.match(confirmSource, /ref=\{cancelButtonRef\}[\s\S]*?onClick=\{onCancel\}/);
  assert.match(confirmSource, /copy\.confirm\.uninstallWarning/);
  // 跳过说明只在确有跳过时渲染。
  assert.match(confirmSource, /skippedTotal > 0 \? <p>\{copy\.confirm\.skipped\(counts\)\}<\/p> : null/);
});

test("弹窗：接管进行中计入 dialogBusy；完成后先刷库再置 installed（与替换安装完成同一套路）", () => {
  assert.match(dialogSource, /const dialogBusy = saving \|\| replacementBusy \|\| externalAdoptBusy;/);
  assert.match(dialogSource, /onBusyChange=\{setExternalAdoptBusy\}/);
  assert.match(dialogSource, /onAdoptCompleted=\{handleExternalAdoptCompleted\}/);
  assert.match(dialogSource, /modName=\{displayModName\}/);
  assert.match(
    dialogSource,
    /const handleExternalAdoptCompleted = useCallback\([\s\S]*?setMessage\(notice\);\s*await onSaved\(\);\s*setReplacementInstallStatus\("installed"\);/,
  );
});
