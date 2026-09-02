import assert from "node:assert/strict";
import { test } from "node:test";
import { projectExternalCardBadge } from "./externalCardBadge.ts";
import { externalStateCopy } from "./externalStateCopy.ts";

const zh = externalStateCopy.zh_cn.badge;

function stateOf(summaryState, counts, extra = {}) {
  return {
    summary: {
      state: summaryState,
      matchedFileCount: counts.matched ?? 0,
      missingFileCount: counts.missing ?? 0,
      changedFileCount: counts.changed ?? 0,
      unreadableFileCount: counts.unreadable ?? 0,
      files: [],
    },
    stale: false,
    lastError: null,
    ...extra,
  };
}

const mixedState = stateOf("mixed", { changed: 2, unreadable: 1, missing: 2 });

test("门禁：只有 manifest 不认领的卡片才显示徽标，残留结果不得上卡片", () => {
  // 已安装/异常态即使 Map 里残留安装前的扫描结果，也必须维持既有状态显示。
  for (const installStatus of ["installed", "disabled", "conflict", "unknown", "repair_required"]) {
    assert.equal(
      projectExternalCardBadge({
        installStatus,
        externalState: mixedState,
        viewMode: "classic",
        copy: zh,
      }),
      null,
      `${installStatus} 不得渲染外部徽标`,
    );
  }
});

test("门禁：本会话没扫过（无记录或无 summary）时不渲染徽标", () => {
  for (const externalState of [null, undefined, stateOf("mixed", {}, { summary: null })]) {
    assert.equal(
      projectExternalCardBadge({
        installStatus: "not_installed",
        externalState,
        viewMode: "classic",
        copy: zh,
      }),
      null,
    );
  }
});

test("档位随视图路由：tech 完整、classic/grid 精简、list 极简", () => {
  const project = (viewMode) =>
    projectExternalCardBadge({
      installStatus: "not_installed",
      externalState: mixedState,
      viewMode,
      copy: zh,
    });

  assert.equal(project("tech").text, "已被改动 · 3 个文件 · 另有 2 个缺失");
  assert.equal(project("classic").text, "已改动 3 · 缺失 2");
  assert.equal(project("grid").text, "已改动 3 · 缺失 2");
  // 极简档只报总数，不假装知道分类。
  assert.equal(project("list").text, "需注意 5");
});

test("label 恒用完整档事实并带外部前缀，与视图档位无关", () => {
  const badge = projectExternalCardBadge({
    installStatus: "not_installed",
    externalState: stateOf("partial", { missing: 3 }),
    viewMode: "list",
    copy: zh,
  });

  assert.equal(badge.text, "需注意 3");
  assert.equal(badge.label, "外部 · 部分安装 · 3 个文件缺失");
  assert.equal(badge.case, "partial");
  assert.equal(badge.stale, false);
});

test("stale 结果在 label 追加过时提示并透传标志", () => {
  const badge = projectExternalCardBadge({
    installStatus: "not_installed",
    externalState: stateOf("changed", { changed: 1 }, { stale: true }),
    viewMode: "grid",
    copy: zh,
  });

  assert.equal(badge.stale, true);
  assert.ok(badge.label.endsWith(` · ${zh.staleHint}`), "label 末尾必须是过时提示");

  const fresh = projectExternalCardBadge({
    installStatus: "not_installed",
    externalState: stateOf("changed", { changed: 1 }),
    viewMode: "grid",
    copy: zh,
  });
  assert.ok(!fresh.label.includes(zh.staleHint), "非 stale 不得出现过时提示");
});

test("case 透传：installed / not_installed / unknown 也产出徽标", () => {
  for (const [state, expectedText] of [
    ["installed", zh.installed],
    ["not_installed", zh.notInstalled],
    ["unknown", zh.unknown],
  ]) {
    const badge = projectExternalCardBadge({
      installStatus: "not_installed",
      externalState: stateOf(state, {}),
      viewMode: "classic",
      copy: zh,
    });
    assert.equal(badge.case, state);
    assert.equal(badge.text, expectedText);
  }
});

test("三语 staleHint 均非空（供 title/aria 合成）", () => {
  for (const locale of ["zh_cn", "en", "ja"]) {
    assert.ok(
      externalStateCopy[locale].badge.staleHint.length > 0,
      `${locale} 缺 staleHint`,
    );
  }
});
