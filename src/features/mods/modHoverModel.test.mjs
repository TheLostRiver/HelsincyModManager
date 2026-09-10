import assert from "node:assert/strict";
import { test } from "node:test";
import { modHoverModel, coherentHoverReplacement } from "./modHoverModel.ts";
import { modOriginLabel } from "./modOriginView.ts";
import { replacementIdentityLabel } from "../replacements/replacementIdentityLabel.ts";

const item = { id: "mod-a", name: "List name", categoryLabels: [{ name: " Armor " }, { name: "Armor" }, { name: "Custom" }] };
const detail = { id: "mod-a", name: "Detailed name", packageId: "pkg-a", description: "  first\n second  ", nexusModId: 42,
  metadata: { author: " Author ", version: " 2.3.4 ", category: "Inferred", tags: ["HD", " HD ", "Custom", " "], dependencies: [] } };

test("hover metadata projects the actual name author version notes categories tags and Nexus ID", () => {
  assert.deepEqual(modHoverModel(detail, item), {
    name: "Detailed name", author: "Author", version: "2.3.4", notes: "first second",
    categories: ["Armor", "Custom"], tags: ["HD", "Custom"], nexusId: "42",
  });
});

test("missing hover metadata never invents an author version or Nexus ID", () => {
  assert.deepEqual(modHoverModel({ ...detail, description: undefined, nexusModId: null, metadata: { tags: [], dependencies: [] } }, { ...item, categoryLabels: [] }), {
    name: "Detailed name", author: null, version: null, notes: null, categories: [], tags: [], nexusId: null,
  });
});

test("hover notes are bounded without losing the original prefix", () => {
  assert.equal(modHoverModel({ ...detail, description: "说明".repeat(100) }, item).notes, `${"说明".repeat(80)}...`);
});

test("unknown and legacy origins are not mislabeled as file imports", () => {
  assert.equal(modOriginLabel(null, "en"), "Unknown origin");
  assert.equal(modOriginLabel({ kind: "unexpected" }, "en"), "Unknown origin");
  assert.notEqual(modOriginLabel({ kind: "migrated_v1" }, "en"), modOriginLabel({ kind: "imported" }, "en"));
});

test("source labels reuse the named adapter without guessing from paths", () => {
  const origin = { kind: "external_import", adapterId: "hunting_box_directory_v1", batchId: "batch", importedAtUnixMillis: null };
  assert.match(modOriginLabel(origin, "zh_cn"), /狩技盒子/);
  assert.doesNotMatch(modOriginLabel({ ...origin, adapterId: "unknown" }, "zh_cn"), /狩技盒子/);
  assert.doesNotMatch(modOriginLabel({ ...origin, importedAtUnixMillis: Number.NaN }, "en"), /Invalid Date/);
});

for (const [locale, name] of [["zh_cn", "测试武器"], ["en", "Fixture weapon"], ["ja", "テスト武器"]]) {
  test(`equipment identity keeps both the localized name and number in ${locale}`, () => {
    assert.equal(replacementIdentityLabel({ internalId: "001", displayNames: { zh_cn: "测试武器", en: "Fixture weapon", ja: "テスト武器" } }, locale), `${name} (001)`);
  });
}

test("unknown equipment names keep their number and declare the missing name", () => {
  assert.equal(replacementIdentityLabel({ internalId: "001" }, "en"), "Unknown name (001)");
});

const summary = { gameId: "mhw", modId: "mod-a", packageId: "pkg-a", sources: [], installedTargets: [] };
test("matching hover responses retain the complete replacement facts", () => {
  assert.equal(coherentHoverReplacement(detail, summary, "mhw"), summary);
});
for (const [name, drift] of [["mod", { modId: "mod-b" }], ["game", { gameId: "other" }], ["package revision", { packageId: "pkg-b" }]]) {
  test(`hover rejects incoherent ${name} replacement facts`, () => {
    assert.equal(coherentHoverReplacement(detail, { ...summary, ...drift }, "mhw"), null);
  });
}
