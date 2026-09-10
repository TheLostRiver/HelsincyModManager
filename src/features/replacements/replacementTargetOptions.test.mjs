import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { buildReplacementTargetOptions, replacementTargetOption } from "./replacementTargetOptions.ts";
import { replacementIdentityLabel } from "./replacementIdentityLabel.ts";

const weapon = {
  id: "model-a", gameId: "mhw", targetType: "weapon", internalId: "two029",
  displayNames: { zh_cn: "黑龙刃", en: "Fatalis Blade", ja: "ミラブレイド" },
  aliases: ["黑龙玄刃", "Black Fatalis Blade", "ブラックミラブレイド"],
  aliasesByLocale: { zh_cn: ["黑龙玄刃"], en: ["Black Fatalis Blade"], ja: ["ブラックミラブレイド"] },
};

test("each weapon name becomes a selectable row with the original model target", () => {
  for (const [locale, names] of [
    ["zh_cn", ["黑龙刃", "黑龙玄刃"]], ["en", ["Fatalis Blade", "Black Fatalis Blade"]],
    ["ja", ["ミラブレイド", "ブラックミラブレイド"]],
  ]) {
    const options = buildReplacementTargetOptions([weapon], locale);
    assert.deepEqual(options.map((option) => option.displayName), names);
    assert.equal(new Set(options.map((option) => option.key)).size, 2);
    assert.ok(options.every((option) => option.target === weapon));
    assert.equal(options[1].secondaryName, undefined, "do not attach the model representative's English name to another weapon");
  }
});

test("search reaches individual current-language and foreign-language names", () => {
  assert.deepEqual(buildReplacementTargetOptions([weapon], "zh_cn", "玄刃").map((option) => option.displayName), ["黑龙玄刃"]);
  assert.deepEqual(buildReplacementTargetOptions([weapon], "zh_cn", "black fatalis").map((option) => option.displayName), ["Black Fatalis Blade"]);
  assert.deepEqual(buildReplacementTargetOptions([weapon], "zh_cn", "blade").map((option) => option.displayName), ["黑龙刃", "Black Fatalis Blade"]);
  assert.equal(buildReplacementTargetOptions([weapon], "zh_cn", "two029").length, 2);
  assert.equal(buildReplacementTargetOptions([weapon], "zh_cn", "not-a-weapon").length, 0);
});

test("armor search aliases remain searchable without becoming duplicate choices", () => {
  const armor = { ...weapon, id: "armor-a", targetType: "armor", internalId: "pl001_0000" };
  assert.equal(buildReplacementTargetOptions([armor], "zh_cn").length, 1);
  assert.equal(buildReplacementTargetOptions([armor], "zh_cn", "玄刃").length, 1);
  assert.equal(buildReplacementTargetOptions([armor], "zh_cn", "pl001_0000").length, 1);
});

test("selected alias keeps its proven name across locale changes and never uses array position as translation", () => {
  const selected = replacementTargetOption(weapon, "en", "黑龙玄刃");
  assert.equal(selected.displayName, "黑龙玄刃");
  assert.equal(selected.target.id, weapon.id);
  assert.equal(selected.key, replacementTargetOption(weapon, "zh_cn", "黑龙玄刃").key);
  assert.equal(replacementIdentityLabel(weapon, "zh_cn", selected.displayName), "黑龙玄刃 (two029)");
  assert.equal(replacementTargetOption(weapon, "en", "unknown name").displayName, "Fatalis Blade");
});

test("missing localized aliases do not cause guessed names, and duplicate labels are not repeated", () => {
  assert.equal(buildReplacementTargetOptions([{ ...weapon, aliasesByLocale: undefined }], "zh_cn").length, 1);
  const duplicate = { ...weapon, aliasesByLocale: { zh_cn: ["黑龙玄刃", "黑龙玄刃", "black guess"] } };
  assert.equal(buildReplacementTargetOptions([duplicate], "zh_cn").length, 2);
});

test("all bundled weapon names are visible while the 601 installation targets remain unique", () => {
  const directory = new URL("../../../src-tauri/crates/hmm-games-mhw/data/weapons/", import.meta.url);
  const targets = readdirSync(directory).filter((name) => /^mhw-weapon-targets\..+\.v1\.json$/.test(name))
    .flatMap((file) => JSON.parse(readFileSync(new URL(file, directory), "utf8")).targets)
    .filter((target) => target.status === "active")
    .map((raw) => ({
      id: raw.stable_id, gameId: "mhw", targetType: "weapon", internalId: raw.internal_id,
      displayNames: Object.fromEntries(Object.entries(raw.names).map(([locale, names]) => [locale, names.display_name])),
      aliases: [...new Set(Object.values(raw.names).flatMap((names) => names.aliases))],
      aliasesByLocale: Object.fromEntries(Object.entries(raw.names).map(([locale, names]) => [locale, names.aliases])),
    }));
  assert.equal(targets.length, 601);
  for (const locale of ["zh_cn", "en", "ja"]) {
    const options = buildReplacementTargetOptions(targets, locale);
    const expected = targets.flatMap((target) => [target.displayNames[locale], ...target.aliasesByLocale[locale]]);
    assert.equal(options.length, 3123, locale);
    assert.deepEqual(options.map((option) => option.displayName).sort(), expected.sort());
    assert.equal(new Set(options.map((option) => option.key)).size, options.length);
    assert.equal(new Set(options.map((option) => option.target.id)).size, 601);
  }
});
