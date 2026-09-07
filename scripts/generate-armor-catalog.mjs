/**
 * 从候选数据生成 MHW:I 防具 retarget catalog。
 *
 * 输入（都不在版本管理里，属于本地候选数据）：
 *   armor-data/equipment.json      槽位路径 -> 简体中文名，272 条
 *   --csv <path>                   多语言名称表，提供 en / ja 与部位标记
 *
 * 输出：
 *   armor-data/generated/mhw-equipment-candidates.armor.v1.json   候选文档，供 validator 审计
 *   src-tauri/crates/hmm-games-mhw/data/mhw-armor-targets.v1.json 运行时 artifact
 *
 * Stable ID 严格按 docs/EQUIPMENT_CATALOG_GOVERNANCE.md 的算法计算，
 * 与 Rust 侧 generate_mhw_equipment_stable_id 必须逐字节一致。
 *
 * 注意：v3 在生成产物上手工补入了 5 条活动/联动装缺失的名称（权利人 Capcom，经
 * kiranico 转录对照）：4 条补 en/ja 展示名；pl057_0010 只补 ja 展示名，其官方英文名
 * 与女版重名，按治理规则记为 en alias（重签记录见 GAME_TERMINOLOGY_SIGNOFF.md）。
 * 重新运行本脚本前必须先把这 5 条并入候选 CSV（pl057_0010 的英文名并入 alias 列而非
 * display name），否则会回退语言覆盖（catalog 键集完备性由 hmm-games-mhw 的测试把关）。
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

const CSV_PATH = args.get("csv");
const REVIEWED_BY = args.get("reviewed-by");
const REVIEWED_AT = args.get("reviewed-at");
if (!CSV_PATH || !REVIEWED_BY || !REVIEWED_AT) {
  console.error(
    "用法: node scripts/generate-armor-catalog.mjs --csv <ArmorData.csv> --reviewed-by <name> --reviewed-at <YYYY-MM-DD>",
  );
  process.exit(2);
}
if (!/^\d{4}-\d{2}-\d{2}$/.test(REVIEWED_AT)) {
  console.error("reviewed-at 必须是 YYYY-MM-DD");
  process.exit(2);
}

const EQUIPMENT_PATH = "armor-data/equipment.json";
/**
 * 每件装备有哪几套模型（`#356`）。由 `tmp/build_variants.py` 从解包的游戏本体枚举，
 * 见该文件的 `source` 字段。
 *
 * **`f_equip` / `m_equip` 不是「女装／男装」，是同一件装备的两套模型。** 实测 272 个槽位
 * 里 262 个两套都有，10 个是单模型的联动装。玩家的角色性别决定游戏加载哪一套——女角穿
 * 只有男模型的「隆」，加载的仍是 `m_equip` 那套。
 */
const VARIANTS_PATH = "armor-data/armor-model-variants.json";
const CANDIDATE_OUT = "armor-data/generated/mhw-equipment-candidates.armor.v1.json";
const ARTIFACT_OUT = "src-tauri/crates/hmm-games-mhw/data/mhw-armor-targets.v1.json";
const SOURCE_ID = "mhw-ingame-equipment-names";
const VARIANT_SOURCE_ID = "mhw-game-assets";
const CATALOG_VERSION = "mhw-armor-v4";

/** 占位条目：治理要求生成 artifact 前显式移除，不能静默变成可选择目标。 */
const DUMMY_NAME = "HARDUMMY";

/** 与 catalog.rs 的 normalize_armor_search_text 同精神，仅用于跨表拼接。 */
const joinKey = (value) =>
  (value ?? "")
    .normalize("NFKC")
    .replace(/[【】[\]（）()·・‧\s]/g, "")
    .toLowerCase();

/** docs/EQUIPMENT_CATALOG_GOVERNANCE.md 的 Stable ID 算法，NUL 分隔后取完整 SHA-256。 */
function stableId(targetKind, pathFamily, resourcePath) {
  const payload = [
    "hmm-mhw-equipment-candidate-v1",
    "mhw",
    targetKind,
    pathFamily,
    resourcePath.toLowerCase(),
  ].join("\0");
  const digest = createHash("sha256").update(payload, "utf8").digest("hex");
  return `mhw:${targetKind}:${digest}`;
}

function parseCsv(text) {
  // 这份文件没有引号包裹字段；名称里不含逗号。若将来变了，这里会明显错位而不是静默出错。
  const lines = text.trim().split(/\r?\n/);
  const header = lines[0].split(",");
  const index = (name) => {
    const at = header.indexOf(name);
    if (at < 0) throw new Error(`CSV 缺少列: ${name}`);
    return at;
  };
  const cols = {
    cn: index("CN 簡体中文"),
    en: index("US English"),
    ja: index("JA 日本語"),
    parts: ["Head", "Chest", "Arm", "Waist", "Leg"].map(index),
  };
  const partNames = ["head", "body", "arms", "waist", "legs"];

  const byName = new Map();
  for (const line of lines.slice(1)) {
    const row = line.split(",");
    const key = joinKey(row[cols.cn]);
    if (!key || byName.has(key)) continue;
    byName.set(key, {
      en: row[cols.en]?.trim() || null,
      ja: row[cols.ja]?.trim() || null,
      parts: cols.parts.map((at, i) => (row[at]?.trim() ? partNames[i] : null)).filter(Boolean),
    });
  }
  return byName;
}

/** 变体能从名称后缀可靠推出；monster / rank / is_full_body 推不出，一律不写。 */
function variantOf(name) {
  if (/阿尔法|α/.test(name)) return "alpha";
  if (/贝塔|β/.test(name)) return "beta";
  if (/伽马|伽玛|γ/.test(name)) return "gamma";
  return null;
}

const equipment = JSON.parse(readFileSync(EQUIPMENT_PATH, "utf8"));
const localized = parseCsv(readFileSync(CSV_PATH, "utf8"));
const previous = JSON.parse(readFileSync(ARTIFACT_OUT, "utf8"));
const variants = JSON.parse(readFileSync(VARIANTS_PATH, "utf8"));

/** internal_id -> 该装备实际存在的模型变体（按 path_family 升序，供输出稳定）。 */
const familiesFor = new Map();
for (const id of variants.shared) familiesFor.set(id, ["pl/f_equip", "pl/m_equip"]);
for (const id of variants.female_only) familiesFor.set(id, ["pl/f_equip"]);
for (const id of variants.male_only) familiesFor.set(id, ["pl/m_equip"]);

// 自指防护：本脚本要从"上一版 artifact"取旧 ID 与旧展示名。
// 若对着自己刚生成的结果再跑一次，每条会把自己的新 hash ID 当成旧 ID，
// 同时原始人工展示名被永久覆盖——静默产出一份看似正常的错数据。
if (previous.catalog_version === CATALOG_VERSION) {
  console.error(
    [
      `拒绝执行：${ARTIFACT_OUT} 已经是 ${CATALOG_VERSION}，再跑会拿生成结果当基线。`,
      `请先 git checkout -- ${ARTIFACT_OUT} 恢复上一版再重试。`,
    ].join("\n"),
  );
  process.exit(2);
}

// 旧 slug ID 必须继续可解析：玩家已安装的 manifest 里存的是它们。
const legacyBySlot = new Map(previous.targets.map((t) => [t.internal_id, t]));

const dropped = [];
const candidates = [];
for (const [resourcePath, zhName] of equipment) {
  const internalId = resourcePath.split("/").pop();
  if (zhName === DUMMY_NAME) {
    dropped.push([resourcePath, zhName, "占位条目"]);
    continue;
  }
  if (!/^pl\d{3}_\d{4}$/.test(internalId)) {
    dropped.push([resourcePath, zhName, "internal_id 形状非法"]);
    continue;
  }

  const extra = localized.get(joinKey(zhName)) ?? null;
  const carriedOver = legacyBySlot.get(internalId);

  const names = { zh_cn: { display_name: zhName, aliases: [] } };
  if (extra?.en) names.en = { display_name: extra.en, aliases: [] };
  if (extra?.ja) names.ja = { display_name: extra.ja, aliases: [] };
  // 扩容不得让已有的检索能力退化。人工 seed 的旧别名要保留；
  // 旧展示名也必须降级成别名——候选数据把「α」写成「阿尔法」，
  // 不保留的话玩家搜「【精英·龙α】服装」会一无所获。
  /*
   * 没有对应 display_name 的别名。
   *
   * `pl057_0010` 的官方英文名与另一件装备逐字重名，按治理规则只能记成 alias，因此它
   * **没有** en display_name。旧写法在 `names[locale]` 缺位时直接 return，别名就被静默
   * 丢掉——v3 是靠在产物上手工补回去的（脚本头部那段警告说的就是这件事）。
   *
   * artifact 里的 aliases 本来就是扁平数组、不带 locale，所以单独收着再合并即可，
   * 不必为了挂别名伪造一个空的 display_name。
   */
  const orphanAliases = [];
  if (carriedOver) {
    const add = (locale, values) => {
      const incoming = values.filter(Boolean);
      if (!names[locale]) {
        orphanAliases.push(...incoming);
        return;
      }
      const merged = new Set([...names[locale].aliases, ...incoming]);
      merged.delete(names[locale].display_name);
      names[locale].aliases = [...merged];
    };
    // 旧展示名按它自己的 locale 归位，不靠字符集猜——
    // 日文名同样含汉字，猜会把它塞进中文别名里。
    for (const [locale, text] of Object.entries(carriedOver.display_name ?? {})) {
      add(locale, [text]);
    }
    // 旧别名没有 locale 标注，只能按字符集分：含汉字归中文，其余归英文。
    const isHan = (text) => /[一-鿿]/.test(text);
    const carriedAliases = carriedOver.aliases ?? [];
    add(
      "zh_cn",
      carriedAliases.filter((alias) => isHan(alias)),
    );
    add(
      "en",
      carriedAliases.filter((alias) => !isHan(alias)),
    );
  }

  /*
   * `#356`：一件装备按它实际存在的模型变体产出 1 或 2 条目标。
   *
   * 变体归属来自游戏本体枚举，不再是硬编码的 `pl/f_equip`。那个常量假设「所有装备都有
   * 女性模型」，结果给 5 件只有男性模型的联动装（隆／杰洛特／巴耶克／里昂／燕尾蝶男）
   * 产出了指向不存在路径的目标——玩家选中、安装成功、游戏不生效。
   *
   * `equipment.json` 里的 `resourcePath` 一律是 `f_equip`，所以按 family 重新构造。
   */
  const families = familiesFor.get(internalId);
  if (!families) {
    dropped.push([resourcePath, zhName, "游戏本体里不存在这个槽位"]);
    continue;
  }

  for (const pathFamily of families) {
    const variantPath = `nativePC/${pathFamily}/${internalId}`;
    candidates.push({
      stable_id: stableId("armor", pathFamily, variantPath),
      target_kind: "armor",
      path_family: pathFamily,
      resource_path: variantPath,
      status: "active",
      // 两个变体是同一件装备，名称与别名逐字相同。治理规则的 display_name 唯一性
      // 因此收敛到「同一 path_family 内唯一」——玩家一次只看得到一个变体
      // （`list_compatible_targets` 按源包的 path_family 筛过）。
      names: structuredClone(names),
      _orphanAliases: [...orphanAliases],
      source_ids: [SOURCE_ID],
      /*
       * 旧 ID 只挂在**继承了旧条目的那一条**上。
       *
       * 旧 catalog 全按 `f_equip` 算 ID，所以：共享装备的 f 变体继承（玩家已安装的
       * manifest 存的就是它），m 变体是全新的；而那 5 件单男模型装备，旧条目虽然算错了
       * family，ID 仍要挂到唯一的 m 变体上，否则旧绑定会解析不了。
       *
       * **必须累积而不是只取上一代的 `id`。** 上一版自己的 `legacy_ids` 里存着更早的
       * slug（AR1 的四条手工种子条目，如 `mhw:armor:fatalis-alpha`），只取 `id` 会让它们
       * 在这一代静默消失——玩家用那些 slug 绑定过的安装会解析不出目标。
       * v2→v3 没暴露这个缺陷，纯粹因为当时的 `id` 本身就是 slug。
       */
      legacy_ids:
        carriedOver && pathFamily === families[0]
          ? [...new Set([carriedOver.id, ...(carriedOver.metadata?.legacy_ids ?? [])])]
          : [],
      _variant: variantOf(zhName),
      _parts: extra?.parts?.length ? extra.parts : null,
      _carried: carriedOver ?? null,
    });
  }
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
    {
      /*
       * `#356`：模型变体归属的来源与名称来源是**两件事**，分开声明。
       *
       * 名称是 Capcom 的游戏术语；变体归属是结构事实（哪个目录存在），从本机安装的游戏
       * 资源枚举得到，不涉及任何第三方转录。
       */
      source_id: VARIANT_SOURCE_ID,
      source_name: "MHW:I game assets (model variant enumeration)",
      source_url: "https://www.monsterhunter.com/world-iceborne/",
      retrieved_at: REVIEWED_AT,
      license: {
        status: "game_terminology",
        rights_holder: "Capcom Co., Ltd.",
        usage: "nominative",
        attribution:
          "Model variant availability is enumerated from a local game installation. This project claims no rights in the game assets and is not affiliated with or endorsed by Capcom.",
        reviewed_by: REVIEWED_BY,
        reviewed_at: REVIEWED_AT,
      },
    },
  ],
  // 只输出 schema 定义的字段；_variant/_parts/_carried 是生成期中间量。
  targets: candidates.map((candidate) => ({
    stable_id: candidate.stable_id,
    target_kind: candidate.target_kind,
    path_family: candidate.path_family,
    resource_path: candidate.resource_path,
    status: candidate.status,
    names: candidate.names,
    source_ids: candidate.source_ids,
    legacy_ids: candidate.legacy_ids,
  })),
};

const artifact = {
  schema_version: 1,
  catalog_version: CATALOG_VERSION,
  game_id: "mhw",
  targets: candidates.map((candidate) => {
    const displayName = {};
    for (const [locale, value] of Object.entries(candidate.names)) {
      displayName[locale] = value.display_name;
    }
    const aliases = [
      ...new Set([
        ...Object.values(candidate.names).flatMap((value) => value.aliases),
        ...candidate._orphanAliases,
      ]),
    ];

    // 只写能诚实得到的元数据。monster / rank / is_full_body 推不出来就不写，
    // adapter 已把它们改成可选（见 validate_armor_metadata）。
    const metadata = { path_family: candidate.path_family };
    const carried = candidate._carried?.metadata ?? {};
    for (const field of ["monster", "rank", "is_full_body"]) {
      if (carried[field] !== undefined) metadata[field] = carried[field];
    }
    const variant = candidate._variant ?? carried.variant ?? null;
    if (variant) metadata.variant = variant;
    const parts = candidate._parts ?? carried.parts ?? null;
    if (parts?.length) metadata.parts = parts;
    if (candidate.legacy_ids.length) metadata.legacy_ids = candidate.legacy_ids;

    return {
      id: candidate.stable_id,
      target_type: "armor",
      display_name: displayName,
      aliases,
      internal_id: candidate.resource_path.split("/").pop(),
      metadata,
    };
  }),
};

mkdirSync(dirname(CANDIDATE_OUT), { recursive: true });
writeFileSync(CANDIDATE_OUT, `${JSON.stringify(candidateDoc, null, 2)}\n`, "utf8");
writeFileSync(ARTIFACT_OUT, `${JSON.stringify(artifact, null, 2)}\n`, "utf8");

const withEn = candidates.filter((c) => c.names.en).length;
const withJa = candidates.filter((c) => c.names.ja).length;
console.log(`输入条目        ${equipment.length}`);
console.log(`剔除            ${dropped.length}`);
for (const [path, name, why] of dropped) console.log(`    ${path}  "${name}"  ${why}`);
console.log(`生成目标        ${candidates.length}`);
console.log(`  含 en         ${withEn}`);
console.log(`  含 ja         ${withJa}`);
console.log(`  仅 zh_cn      ${candidates.length - withEn}`);
console.log(`  带 legacy_ids ${candidates.filter((c) => c.legacy_ids.length).length}`);
console.log(`候选文档        ${CANDIDATE_OUT}`);
console.log(`运行时 artifact ${ARTIFACT_OUT}`);
