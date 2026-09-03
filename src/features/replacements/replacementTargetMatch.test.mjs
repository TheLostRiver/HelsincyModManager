import assert from "node:assert/strict";
import { test } from "node:test";
import { replacementCopy } from "./replacementCopy.ts";
import {
  REPLACEMENT_TARGET_MATCH_HINT_LIMIT,
  matchedHiddenReplacementTargetNames,
} from "./replacementTargetMatch.ts";

// 夹具取自 bundled 武器 artifact（two029 / bs_two001）。DTO 的 aliases 是后端把三语别名
// 压平后的 BTreeSet（ASCII → 片假名 → 汉字的字节序），这里照同一形状构造，别按语言分组。
const fatalisBlade = {
  displayNames: { en: "Fatalis Blade", ja: "ミラブレイド", zh_cn: "黑龙刃" },
  aliases: ["Black Fatalis Blade", "ブラックミラブレイド", "黑龙玄刃"],
};
const fatalisBladeZh = { displayName: "黑龙刃", secondaryName: "Fatalis Blade" };

const frostBlaze = {
  displayNames: { en: "Frost Blaze I", ja: "フロストブレイズⅠ", zh_cn: "霜炎1" },
  aliases: [
    "Datura Blaze I",
    "Datura Blaze II",
    "Datura Blaze III",
    "Frost Blaze II",
    "Frost Blaze III",
    "ダチュラブレイズⅠ",
    "フロストブレイズⅡ",
    "フロストブレイズⅢ",
    "曼陀罗之炎1",
    "曼陀罗之炎2",
    "曼陀罗之炎3",
    "霜炎2",
    "霜炎3",
  ],
};
const frostBlazeZh = { displayName: "霜炎1", secondaryName: "Frost Blaze I" };

test("空关键词永不产生提示，即使这一行什么名字都没渲染", () => {
  const nothingRendered = { displayName: "" };
  assert.equal(matchedHiddenReplacementTargetNames(fatalisBlade, nothingRendered, ""), null);
  assert.equal(matchedHiddenReplacementTargetNames(fatalisBlade, nothingRendered, "   "), null);
});

test("关键词已命中行内展示名时不提示，哪怕别名也命中", () => {
  // 「黑龙」同时命中展示名「黑龙刃」与别名「黑龙玄刃」——行里已经看得见，不该再多一行。
  assert.equal(matchedHiddenReplacementTargetNames(fatalisBlade, fatalisBladeZh, "黑龙"), null);
});

test("关键词已命中行内英文副名时不提示", () => {
  // 中文界面下英文副名 Fatalis Blade 是渲染出来的；别名 Black Fatalis Blade 也命中但不必提示。
  assert.equal(matchedHiddenReplacementTargetNames(fatalisBlade, fatalisBladeZh, "fatalis"), null);
});

test("只命中别名时给出该别名（终阶名搜到初阶行的场景）", () => {
  assert.deepEqual(matchedHiddenReplacementTargetNames(fatalisBlade, fatalisBladeZh, "玄刃"), {
    names: ["黑龙玄刃"],
    hiddenCount: 0,
  });
});

test("命中其他语言的展示名也提示，且展示名排在别名前", () => {
  // 中文界面搜日文名：ja 展示名「ミラブレイド」与别名「ブラックミラブレイド」都命中。
  assert.deepEqual(
    matchedHiddenReplacementTargetNames(fatalisBlade, fatalisBladeZh, "ミラブレイド"),
    { names: ["ミラブレイド", "ブラックミラブレイド"], hiddenCount: 0 },
  );
});

test("命中数超过上限时截断并计数，上限可调", () => {
  assert.equal(REPLACEMENT_TARGET_MATCH_HINT_LIMIT, 2);
  assert.deepEqual(matchedHiddenReplacementTargetNames(frostBlaze, frostBlazeZh, "曼陀罗"), {
    names: ["曼陀罗之炎1", "曼陀罗之炎2"],
    hiddenCount: 1,
  });
  assert.deepEqual(matchedHiddenReplacementTargetNames(frostBlaze, frostBlazeZh, "曼陀罗", 3), {
    names: ["曼陀罗之炎1", "曼陀罗之炎2", "曼陀罗之炎3"],
    hiddenCount: 0,
  });
});

test("同一个名字既是其他语言展示名又是别名时只出现一次", () => {
  const target = {
    displayNames: { en: "Beta", ja: "Gamma", zh_cn: "Alpha" },
    aliases: ["Gamma", "Gamma II"],
  };
  assert.deepEqual(
    matchedHiddenReplacementTargetNames(target, { displayName: "Alpha", secondaryName: "Beta" }, "gamma"),
    { names: ["Gamma", "Gamma II"], hiddenCount: 0 },
  );
});

test("命中判据与列表过滤一致：大小写不敏感的子串匹配", () => {
  // 「frost blaze ii」不是副名「Frost Blaze I」的子串，但同时是 II 与 III 两个别名的子串。
  assert.deepEqual(
    matchedHiddenReplacementTargetNames(frostBlaze, frostBlazeZh, "FROST BLAZE II"),
    { names: ["Frost Blaze II", "Frost Blaze III"], hiddenCount: 0 },
  );
});

test("匹配提示文案三语各按本语言的顿号 / 逗号连接，截断计数另起一段", () => {
  const names = ["曼陀罗之炎1", "曼陀罗之炎2"];
  assert.equal(replacementCopy.zh_cn.panel.matchedNames(names), "匹配：曼陀罗之炎1、曼陀罗之炎2");
  assert.equal(replacementCopy.zh_cn.panel.matchedNamesMore(1), "+1 个");
  assert.equal(
    replacementCopy.en.panel.matchedNames(["Frost Blaze II", "Frost Blaze III"]),
    "Matches: Frost Blaze II, Frost Blaze III",
  );
  assert.equal(replacementCopy.en.panel.matchedNamesMore(3), "+3 more");
  assert.equal(
    replacementCopy.ja.panel.matchedNames(["フロストブレイズⅡ", "フロストブレイズⅢ"]),
    "一致：フロストブレイズⅡ、フロストブレイズⅢ",
  );
  assert.equal(replacementCopy.ja.panel.matchedNamesMore(2), "+2 件");
});
