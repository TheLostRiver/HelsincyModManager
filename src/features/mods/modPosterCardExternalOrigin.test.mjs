import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { modLibraryCopy } from "./modLibraryCopy.ts";
import { externalImportCopy } from "./external-import/externalImportCopy.ts";
import { modLibraryItems } from "./modsLibraryData.ts";

const currentDirectory = dirname(fileURLToPath(import.meta.url));
const cardSource = readFileSync(join(currentDirectory, "ModPosterCard.tsx"), "utf8");

test("外部来源短标只在 externalImportAdapterId 存在时渲染，且带全量 title/aria", () => {
  // 源码形状断言：门禁表达式与无障碍属性缺一不可。
  assert.match(cardSource, /item\.externalImportAdapterId\s*\?/);
  assert.match(cardSource, /mod-card__status-origin/);
  assert.match(cardSource, /title=\{externalOrigin\.title\}/);
  assert.match(cardSource, /aria-label=\{externalOrigin\.title\}/);
});

test("adapter 展示名单一出处：卡片从 externalImportCopy 取词，不自带映射", () => {
  assert.match(
    cardSource,
    /externalImportHistory\.adapters|externalAdapterLabels/,
    "卡片必须经 externalImportCopy 的 adapters 字典取展示名",
  );
  for (const locale of ["zh_cn", "en", "ja"]) {
    assert.ok(
      externalImportCopy[locale].history.adapters.hunting_box_directory_v1.length > 0,
      `${locale} 缺狩技盒子展示名`,
    );
  }
});

test("externalOriginTitle 三语都把 adapter 展示名织进全量说明", () => {
  const zh = modLibraryCopy.zh_cn.card.externalOriginTitle("狩技盒子");
  assert.equal(zh, "外部来源：狩技盒子");
  for (const locale of ["zh_cn", "en", "ja"]) {
    const card = modLibraryCopy[locale].card;
    assert.ok(card.externalOriginShort.length > 0, `${locale} 短标不得为空`);
    assert.match(
      card.externalOriginTitle("PROBE_LABEL"),
      /PROBE_LABEL/,
      `${locale} 的 title 必须包含 adapter 展示名`,
    );
  }
});

test("mock 数据里混有外部来源条目，dev 模式能看见短标", () => {
  const externalCount = modLibraryItems.filter(
    (item) => item.externalImportAdapterId === "hunting_box_directory_v1",
  ).length;
  assert.ok(externalCount > 0, "生成器必须至少产出一个外部来源条目");
});
