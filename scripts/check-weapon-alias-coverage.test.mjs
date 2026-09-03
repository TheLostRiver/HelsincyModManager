import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  collectArtifactNames,
  collectPathTableNames,
  collectReferenceNames,
  compareCoverage,
  normalizeName,
  renderReport,
} from "./check-weapon-alias-coverage.mjs";

const scriptsDir = dirname(fileURLToPath(import.meta.url));
const script = join(scriptsDir, "check-weapon-alias-coverage.mjs");
const cleanups = [];

test.after(() => {
  for (const dir of cleanups) {
    rmSync(dir, { recursive: true, force: true });
  }
});

// ---- fixtures (shapes copied from armor-data/*.json and the bundled shards) ----

function target(internalId, names, status = "active") {
  return { internal_id: internalId, status, names };
}

const fatalis = target("two029", {
  zh_cn: { display_name: "黑龙刃", aliases: ["黑龙玄刃"] },
  en: { display_name: "Fatalis Blade", aliases: ["Black Fatalis Blade"] },
  ja: { display_name: "ミラブレイド", aliases: ["ブラックミラブレイド"] },
});

const reference = {
  A1: { zh_cn: "黑龙刃", en: "Fatalis Blade", ja: "ミラブレイド" },
  A2: { zh_cn: "黑龙玄刃", en: "Black Fatalis Blade", ja: "ブラックミラブレイド" },
};

const pathTable = {
  大剑: { 黑龙刃: "nativePC/wp/two/two029", 黑龙玄刃: "nativePC/wp/two/two029", HARDUMMY: "nativePC/wp/two/two999" },
};

test("normalizeName 与生成器的 norm 完全一致：NFKC、去括号与点号空白、小写", () => {
  assert.equal(normalizeName("Ｆrost　Blaze Ⅰ"), "frostblazei");
  assert.equal(normalizeName("爆热机关式【银翼】"), "爆热机关式银翼");
  assert.equal(normalizeName("Wyvern Ignition \"Impact\""), "wyvernignition\"impact\"");
  assert.equal(normalizeName("（A）[b]·c・d‧e"), "abcde");
  assert.equal(normalizeName(null), "");

  // 两份源码里的 norm 表达式必须逐字相同：catalog 是用那把钥匙生成的，这里换一把就会报假缺口。
  const extract = (source, label) => {
    const match = source.match(/\.normalize\("NFKC"\)\.replace\(\/\[[^\n]*?\/g, ""\)\.toLowerCase\(\)/);
    assert.ok(match, `${label} 必须包含 norm 表达式`);
    return match[0];
  };
  assert.equal(
    extract(readFileSync(script, "utf8"), "check-weapon-alias-coverage.mjs"),
    extract(readFileSync(join(scriptsDir, "generate-weapon-catalog.mjs"), "utf8"), "generate-weapon-catalog.mjs"),
  );
});

test("artifact 名称集 = 活跃目标的展示名 + 别名，按语言分开；退役目标不计", () => {
  const retired = target(
    "two998",
    { zh_cn: { display_name: "已退役", aliases: ["退役别名"] } },
    "retired",
  );
  const names = collectArtifactNames([fatalis, retired]);
  assert.deepEqual([...names.zh_cn.values()], ["黑龙刃", "黑龙玄刃"]);
  assert.deepEqual([...names.en.values()], ["Fatalis Blade", "Black Fatalis Blade"]);
  assert.deepEqual([...names.ja.values()], ["ミラブレイド", "ブラックミラブレイド"]);
});

test("参考名称集按语言归位，缺语言或空条目不报错；首个原文拼写保留供展示", () => {
  const names = collectReferenceNames({
    ...reference,
    B1: { zh_cn: "黑龍刃" },
    B2: null,
    B3: { en: "  " },
  });
  // 「黑龍刃」与「黑龙刃」不同字，是两个名字；空白 en 被丢弃。
  assert.deepEqual([...names.zh_cn.values()], ["黑龙刃", "黑龙玄刃", "黑龍刃"]);
  assert.deepEqual([...names.en.values()], ["Fatalis Blade", "Black Fatalis Blade"]);
  assert.deepEqual([...names.ja.values()], ["ミラブレイド", "ブラックミラブレイド"]);
});

test("weapon.json 的中文名集合剔除游戏占位名 HARDUMMY", () => {
  assert.deepEqual([...collectPathTableNames(pathTable).values()], ["黑龙刃", "黑龙玄刃"]);
});

test("覆盖比对：缺失与多出都算缺口，weapon.json 交叉核对同样参与判定", () => {
  const complete = compareCoverage({
    reference: collectReferenceNames(reference),
    artifact: collectArtifactNames([fatalis]),
    pathTable: collectPathTableNames(pathTable),
  });
  assert.equal(complete.ok, true);
  assert.deepEqual(complete.locales.zh_cn, {
    referenceCount: 2,
    artifactCount: 2,
    missing: [],
    extra: [],
  });
  assert.deepEqual(complete.pathTable, { count: 2, missingFromReference: [], missingFromArtifact: [] });

  // artifact 少了中文别名：zh_cn 缺 1，weapon.json 交叉核对也指出同一个名字。
  const withoutAlias = target("two029", {
    ...fatalis.names,
    zh_cn: { display_name: "黑龙刃", aliases: [] },
  });
  const gap = compareCoverage({
    reference: collectReferenceNames(reference),
    artifact: collectArtifactNames([withoutAlias]),
    pathTable: collectPathTableNames(pathTable),
  });
  assert.equal(gap.ok, false);
  assert.deepEqual(gap.locales.zh_cn.missing, ["黑龙玄刃"]);
  assert.deepEqual(gap.locales.en.missing, []);
  assert.deepEqual(gap.pathTable.missingFromArtifact, ["黑龙玄刃"]);

  // artifact 多出参考表没有的名字：不是「缺口」但同样破坏「与官方名 1:1」，判 FAIL。
  const withExtra = target("two029", {
    ...fatalis.names,
    en: { display_name: "Fatalis Blade", aliases: ["Black Fatalis Blade", "Community Nickname"] },
  });
  const extra = compareCoverage({
    reference: collectReferenceNames(reference),
    artifact: collectArtifactNames([withExtra]),
  });
  assert.equal(extra.ok, false);
  assert.deepEqual(extra.locales.en.extra, ["Community Nickname"]);
  assert.deepEqual(extra.locales.en.missing, []);

  // 报告：每语一行 PASS/FAIL，样例受 samples 截断并标出余量。
  const lines = renderReport(gap, { samples: 0 });
  assert.match(lines[1], /^FAIL {2}zh_cn: reference 2 names, artifact 1 names, missing 1, extra 0$/);
  assert.match(lines[2], /missing \(reference has, artifact lacks\): … \+1$/);
  assert.match(lines[3], /^PASS {2}en:/);
  assert.equal(lines.at(-1), "coverage gaps found");
});

// ---- CLI ----

function writeFixture({ names = reference, weapon = pathTable, targets = [fatalis] } = {}) {
  const root = mkdtempSync(join(tmpdir(), "hmm-alias-cov-"));
  cleanups.push(root);
  const armorData = join(root, "armor-data");
  const artifact = join(root, "weapons");
  mkdirSync(armorData);
  mkdirSync(artifact);
  if (names !== null) {
    writeFileSync(join(armorData, "weapon-names.json"), JSON.stringify(names));
  }
  if (weapon !== null) {
    writeFileSync(join(armorData, "weapon.json"), JSON.stringify(weapon));
  }
  writeFileSync(
    join(artifact, "mhw-weapon-targets.two.v1.json"),
    JSON.stringify({ schema_version: 1, catalog_version: "mhw-weapon-v1", game_id: "mhw", targets }),
  );
  return { armorData, artifact };
}

function run({ armorData, artifact }, args = []) {
  return spawnSync(process.execPath, [script, ...args], {
    encoding: "utf8",
    env: { ...process.env, HMM_ARMOR_DATA_DIR: armorData, HMM_WEAPON_ARTIFACT_DIR: artifact },
  });
}

test("CLI：参考数据缺失时退出码 2，并告知去哪里重新抓取", () => {
  const result = run(writeFixture({ names: null }));
  assert.equal(result.status, 2);
  assert.match(result.stderr, /reference data not found: .*weapon-names\.json/);
  assert.match(result.stderr, /armor-data\/scripts\/fetch-weapon-names\.mjs/);
  assert.match(result.stderr, /HMM_ARMOR_DATA_DIR/);
});

test("CLI：全覆盖时退出码 0 并逐语言 PASS；有缺口时退出码 1 且 --json 可机读", () => {
  const complete = run(writeFixture());
  assert.equal(complete.status, 0, complete.stderr);
  assert.match(complete.stdout, /PASS {2}zh_cn: reference 2 names, artifact 2 names, missing 0, extra 0/);
  assert.match(complete.stdout, /coverage complete: no gaps in any locale/);

  const gapFixture = writeFixture({
    targets: [target("two029", { ...fatalis.names, ja: { display_name: "ミラブレイド", aliases: [] } })],
  });
  const gap = run(gapFixture, ["--json"]);
  assert.equal(gap.status, 1, gap.stderr);
  const parsed = JSON.parse(gap.stdout);
  assert.equal(parsed.ok, false);
  assert.deepEqual(parsed.locales.ja.missing, ["ブラックミラブレイド"]);
  assert.deepEqual(parsed.locales.zh_cn.missing, []);
});

test("CLI：未知参数与非法 --samples 退出码 2", () => {
  const fixture = writeFixture();
  assert.equal(run(fixture, ["--bogus"]).status, 2);
  assert.equal(run(fixture, ["--samples", "-1"]).status, 2);
  assert.equal(run(fixture, ["--samples", "0"]).status, 0);
});
