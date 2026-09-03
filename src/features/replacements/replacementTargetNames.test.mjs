import assert from "node:assert/strict";
import { test } from "node:test";
import { replacementCopy } from "./replacementCopy.ts";
import {
  replacementTargetSearchValues,
  resolveReplacementTargetAliases,
  resolveReplacementTargetNames,
} from "./replacementTargetNames.ts";

// 夹具取自 bundled 武器 artifact 的 two029（黑龙刃）。
const displayNames = { en: "Fatalis Blade", ja: "ミラブレイド", zh_cn: "黑龙刃" };
const aliasesByLocale = {
  en: ["Black Fatalis Blade"],
  ja: ["ブラックミラブレイド"],
  zh_cn: ["黑龙玄刃"],
};

test("展示名按当前语言取，英文副名只在与主名不同时出现", () => {
  assert.deepEqual(resolveReplacementTargetNames(displayNames, "zh_cn"), {
    displayName: "黑龙刃",
    secondaryName: "Fatalis Blade",
  });
  assert.deepEqual(resolveReplacementTargetNames(displayNames, "ja"), {
    displayName: "ミラブレイド",
    secondaryName: "Fatalis Blade",
  });
  // 英文界面：主名就是英文，不再重复一遍副名。
  assert.deepEqual(resolveReplacementTargetNames(displayNames, "en"), {
    displayName: "Fatalis Blade",
    secondaryName: undefined,
  });
});

test("展示名 fallback 链：locale → fallback → en → 任一可用 → 空串", () => {
  // ja 缺席时沿 localeMeta.ja.fallback（en）回落，副名与主名相同故省略。
  assert.deepEqual(resolveReplacementTargetNames({ en: "Fatalis Blade", zh_cn: "黑龙刃" }, "ja"), {
    displayName: "Fatalis Blade",
    secondaryName: undefined,
  });
  // 连 en 都没有：取任一可用的名字。
  assert.deepEqual(resolveReplacementTargetNames({ zh_cn: "黑龙刃" }, "en"), {
    displayName: "黑龙刃",
    secondaryName: undefined,
  });
  assert.deepEqual(resolveReplacementTargetNames({}, "zh_cn"), {
    displayName: "",
    secondaryName: undefined,
  });
});

test("本语言别名按当前语言取，缺席或整表缺席返回空", () => {
  assert.deepEqual(resolveReplacementTargetAliases(aliasesByLocale, "zh_cn"), ["黑龙玄刃"]);
  assert.deepEqual(resolveReplacementTargetAliases(aliasesByLocale, "ja"), ["ブラックミラブレイド"]);
  assert.deepEqual(resolveReplacementTargetAliases(aliasesByLocale, "en"), ["Black Fatalis Blade"]);
  // 铠甲 catalog：DTO 省略键 → 前端拿到 undefined → 空表，不显示计数与摘要。
  assert.deepEqual(resolveReplacementTargetAliases(undefined, "zh_cn"), []);
});

test("别名 fallback 链与展示名相同（locale → fallback → en），但空表就停、不拿英文顶上", () => {
  // ja 键缺席：回落 en。
  assert.deepEqual(
    resolveReplacementTargetAliases({ en: ["Black Fatalis Blade"], zh_cn: ["黑龙玄刃"] }, "ja"),
    ["Black Fatalis Blade"],
  );
  // ja 键存在但为空：这个语言确实没有别名，返回空而不是英文别名。
  assert.deepEqual(
    resolveReplacementTargetAliases({ en: ["Black Fatalis Blade"], ja: [] }, "ja"),
    [],
  );
  // 只有 zh_cn 时，英文界面不做「任一可用」兜底。
  assert.deepEqual(resolveReplacementTargetAliases({ zh_cn: ["黑龙玄刃"] }, "en"), []);
});

test("别名计数文案三语：药丸带 + 号，摘要计数不带；英文按数量变复数", () => {
  assert.equal(replacementCopy.zh_cn.panel.aliasCount(19), "+19 个名称");
  assert.equal(replacementCopy.zh_cn.panel.selectedAliasesCount(19), "19 个名称");
  assert.equal(replacementCopy.en.panel.aliasCount(1), "+1 name");
  assert.equal(replacementCopy.en.panel.aliasCount(19), "+19 names");
  assert.equal(replacementCopy.en.panel.selectedAliasesCount(1), "1 name");
  assert.equal(replacementCopy.en.panel.selectedAliasesCount(19), "19 names");
  assert.equal(replacementCopy.ja.panel.aliasCount(19), "+19 件の名称");
  assert.equal(replacementCopy.ja.panel.selectedAliasesCount(19), "19 件の名称");
  // 摘要要说清「共用模型 → 外观一同改变」这层事实，三语都得有这一句。
  for (const locale of ["zh_cn", "en", "ja"]) {
    assert.ok(replacementCopy[locale].panel.selectedAliasesHint.length > 0, locale);
    assert.ok(replacementCopy[locale].panel.aliasCountTitle.length > 0, locale);
  }
});

test("检索值集合 = 全部语言的展示名，与界面语言无关", () => {
  assert.deepEqual(replacementTargetSearchValues(displayNames).sort(), [
    "Fatalis Blade",
    "ミラブレイド",
    "黑龙刃",
  ]);
});
