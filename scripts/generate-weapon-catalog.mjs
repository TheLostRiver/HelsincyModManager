/**
 * 从候选数据生成 MHW:I 武器 retarget catalog（WR-02B）。
 *
 * 输入（都不在版本管理里，属于本地候选数据）：
 *   armor-data/weapon.json         中文名 -> 模型路径，14 类 3125 名 / 603 路径
 *   armor-data/weapon-names.json   不透明ID -> { en, zh_cn, ja }，三语名称表
 *
 * 输出：
 *   armor-data/generated/mhw-equipment-candidates.weapon.v1.json  候选文档，供 validator 审计
 *   src-tauri/crates/hmm-games-mhw/data/mhw-weapon-targets.v1.json 运行时 artifact
 *
 * 建模要点（WR-01 已定，勿改）：
 * - 重定向目标是**模型路径**不是武器：603 条路径下挂着 3125 个武器名，
 *   同一路径最多 48 个名。必须建成「稳定 target + aliases」，不是重复安装目标。
 * - 展示名取**树根**（剥掉尾部数字后最短的词干），其余名称全部进 aliases 供搜索。
 *   不取终阶名：601 个目标里 130 个只有一个名字；有别名的 471 个中只有 2 个是单一升级线，
 *   其余 469 个都是多条武器线共用一个模型，「终阶」根本不唯一，而且我们手上是扁平的
 *   名称->路径映射，没有升级树的父子边（2026-09-03 按 bundled 分片复算，见 #274）。
 *
 * 许可：名称属于卡普空，按 game_terminology 状态如实声明，不主张任何权利。
 * 政策依据见 EQUIPMENT_CATALOG_GOVERNANCE.md 的「关于 game_terminology 的政策决定」。
 */
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

const args = new Map();
for (let i = 2; i < process.argv.length; i += 2) {
  args.set(process.argv[i].replace(/^--/, ""), process.argv[i + 1]);
}
const REVIEWED_BY = args.get("reviewed-by");
const REVIEWED_AT = args.get("reviewed-at");
if (!REVIEWED_BY || !/^\d{4}-\d{2}-\d{2}$/.test(REVIEWED_AT ?? "")) {
  console.error(
    "用法: node scripts/generate-weapon-catalog.mjs --reviewed-by <name> --reviewed-at <YYYY-MM-DD>",
  );
  process.exit(2);
}

const LOCAL_WEAPONS = "armor-data/weapon.json";
const LOCAL_NAMES = "armor-data/weapon-names.json";
const CANDIDATE_OUT = "armor-data/generated/mhw-equipment-candidates.weapon.v1.json";
const ARTIFACT_DIR = "src-tauri/crates/hmm-games-mhw/data/weapons";
const SOURCE_ID = "mhw-ingame-equipment-names";
const CATALOG_VERSION = "mhw-weapon-v1";
const DUMMY_NAME = "HARDUMMY";
const LOCALES = ["zh_cn", "en", "ja"];

const norm = (value) =>
  (value ?? "").normalize("NFKC").replace(/[【】[\]（）()·・‧\s]/g, "").toLowerCase();

/** docs/EQUIPMENT_CATALOG_GOVERNANCE.md 的 Stable ID 算法，NUL 分隔后取完整 SHA-256。 */
function stableId(targetKind, pathFamily, resourcePath) {
  const payload = [
    "hmm-mhw-equipment-candidate-v1",
    "mhw",
    targetKind,
    pathFamily,
    resourcePath.toLowerCase(),
  ].join("\0");
  return `mhw:${targetKind}:${createHash("sha256").update(payload, "utf8").digest("hex")}`;
}

/** 树根：剥掉尾部数字后最短的词干；同长取字典序最小，保证可复现。 */
function rootName(names) {
  const stems = [...new Set(names.map((name) => name.replace(/\d+$/, "")))].sort(
    (a, b) => a.length - b.length || a.localeCompare(b),
  );
  const stem = stems[0];
  return (
    names.find((name) => name === stem) ??
    names.filter((name) => name.replace(/\d+$/, "") === stem).sort()[0] ??
    names[0]
  );
}

const localized = new Map();
for (const entry of Object.values(JSON.parse(readFileSync(LOCAL_NAMES, "utf8")))) {
  const key = norm(entry.zh_cn);
  if (key && !localized.has(key)) localized.set(key, entry);
}

const local = JSON.parse(readFileSync(LOCAL_WEAPONS, "utf8"));
const pathNames = new Map();
for (const entries of Object.values(local)) {
  for (const [name, path] of Object.entries(entries)) {
    if (!pathNames.has(path)) pathNames.set(path, []);
    pathNames.get(path).push(name);
  }
}

const dropped = [];
const targets = [];
for (const [resourcePath, names] of [...pathNames.entries()].sort(([a], [b]) => a.localeCompare(b))) {
  const real = names.filter((name) => name !== DUMMY_NAME);
  if (real.length === 0) {
    // 治理要求：生成 artifact 前显式移除 dummy。武器 catalog 解析器更严，
    // 见到 status=dummy 会直接拒绝整份文档。
    dropped.push([resourcePath, "全部为占位条目"]);
    continue;
  }

  const shape = resourcePath.match(/^nativePC\/wp\/([a-z]+)\/([a-z0-9_]+)$/);
  if (!shape) {
    dropped.push([resourcePath, "路径形态不合规"]);
    continue;
  }
  const [, family, internalId] = shape;
  const pathFamily = `wp/${family}`;

  const root = rootName(real);
  const rootEntry = localized.get(norm(root)) ?? null;

  // 展示名按 locale 取；别名同样按 locale 归位，不混成一锅。
  const localeNames = {};
  for (const locale of LOCALES) {
    const display = locale === "zh_cn" ? root : rootEntry?.[locale];
    if (!display) continue;
    const aliases = new Set();
    for (const name of real) {
      if (name === root) continue;
      const entry = localized.get(norm(name));
      const alias = locale === "zh_cn" ? name : entry?.[locale];
      if (alias && alias !== display) aliases.add(alias);
    }
    localeNames[locale] = { display_name: display, aliases: [...aliases] };
  }

  targets.push({
    stable_id: stableId("weapon", pathFamily, resourcePath),
    target_type: "weapon",
    resource_path: resourcePath,
    internal_id: internalId,
    metadata: { family, path_family: pathFamily },
    status: "active",
    names: localeNames,
    legacy_ids: [],
  });
}

// 按 family 分片：全量单文件 693KB / 24612 行，超出 policy 的 256KB / 10000 行硬限。
// family 是领域边界（跨 family 重定向被禁），拆在这里最自然；运行时由
// MhwWeaponCatalogSource::parse_sharded 合并后单次校验，跨分片检查不打折。
const shards = new Map();
for (const target of targets) {
  const family = target.metadata.family;
  if (!shards.has(family)) shards.set(family, []);
  shards.get(family).push(target);
}

const candidateDoc = {
  schema_version: 1,
  catalog_version: CATALOG_VERSION,
  game_id: "mhw",
  sources: [
    {
      source_id: SOURCE_ID,
      source_name: "MHW:I in-game equipment names",
      source_url: "https://www.monsterhunter.com/world-iceborne/",
      retrieved_at: REVIEWED_AT,
      license: {
        status: "game_terminology",
        rights_holder: "Capcom Co., Ltd.",
        usage: "nominative",
        attribution:
          "Equipment names are trademarks and content of Capcom Co., Ltd. This project claims no rights in them and is not affiliated with or endorsed by Capcom.",
        reviewed_by: REVIEWED_BY,
        reviewed_at: REVIEWED_AT,
      },
    },
  ],
  targets: targets.map((target) => ({
    stable_id: target.stable_id,
    target_kind: "weapon",
    path_family: target.metadata.path_family,
    resource_path: target.resource_path,
    status: target.status,
    names: target.names,
    source_ids: [SOURCE_ID],
    legacy_ids: target.legacy_ids,
  })),
};

mkdirSync(dirname(CANDIDATE_OUT), { recursive: true });
writeFileSync(CANDIDATE_OUT, `${JSON.stringify(candidateDoc, null, 2)}\n`, "utf8");

mkdirSync(ARTIFACT_DIR, { recursive: true });
const shardSizes = [];
for (const [family, familyTargets] of [...shards.entries()].sort(([a], [b]) => a.localeCompare(b))) {
  const shard = {
    schema_version: 1,
    catalog_version: CATALOG_VERSION,
    game_id: "mhw",
    targets: familyTargets,
  };
  const text = `${JSON.stringify(shard, null, 2)}\n`;
  const file = `${ARTIFACT_DIR}/mhw-weapon-targets.${family}.v1.json`;
  writeFileSync(file, text, "utf8");
  shardSizes.push([family, familyTargets.length, Buffer.byteLength(text)]);
}

const triCount = targets.filter((t) => LOCALES.every((l) => t.names[l])).length;
const aliasCount = targets.reduce(
  (sum, t) => sum + LOCALES.reduce((n, l) => n + (t.names[l]?.aliases.length ?? 0), 0),
  0,
);
console.log(`输入路径        ${pathNames.size}`);
console.log(`剔除            ${dropped.length}`);
for (const [path, why] of dropped) console.log(`    ${path}  ${why}`);
console.log(`生成目标        ${targets.length}`);
console.log(`  三语齐全      ${triCount}`);
console.log(`  别名总量      ${aliasCount}`);
console.log(`候选文档        ${CANDIDATE_OUT}`);
console.log(`运行时分片      ${shardSizes.length} 份 -> ${ARTIFACT_DIR}/`);
const biggest = shardSizes.reduce((a, b) => (b[2] > a[2] ? b : a));
for (const [family, count, bytes] of shardSizes) {
  console.log(`    ${family.padEnd(5)} ${String(count).padStart(3)} 条  ${String(Math.round(bytes / 1024)).padStart(3)}KB`);
}
console.log(`  最大分片      ${biggest[0]} ${Math.round(biggest[2] / 1024)}KB  (硬限 256KB)`);
