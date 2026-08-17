import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "..");
const defaultOutputPath = resolve(repoRoot, "src/features/mods/modsLibraryData.ts");

const statusCycle = /** @type {const} */ (["installed", "disabled", "conflict"]);
const categoryCycle = /** @type {const} */ ([
  ["外观"],
  ["防具替换"],
  ["武器替换"],
  ["语音替换"],
  ["工具 / 前置"],
  ["外观", "防具替换"],
  ["外观", "武器替换"],
  ["语音替换", "工具 / 前置"],
]);
const gradientPairs = /** @type {const} */ ([
  ["#d7e7ff", "#77a8ff"],
  ["#eeeff3", "#b5c2d6"],
  ["#e0f0dc", "#7cc47c"],
  ["#fbe9cb", "#e8b15a"],
  ["#f5e6f2", "#d98bc7"],
  ["#e5e7ff", "#9ca3ff"],
  ["#eef2ff", "#a5b4fc"],
  ["#e2f7ec", "#86efac"],
  ["#fff1de", "#fdba74"],
  ["#fce7f3", "#f9a8d4"],
  ["#ede9fe", "#a78bfa"],
  ["#dcfce7", "#4ade80"],
]);

const seedNames = [
  "非官方仪式礼服",
  "盛夏兔女郎套装",
  "包臀裙重制",
  "贵妇礼裙",
  "薄纱晚宴长裙",
  "夜宴礼装",
  "雪狐披肩",
  "礼宾接待套装",
  "月白礼服",
  "祭典洋装",
  "月辰剑纹重涂",
  "公会纹章武器翻新",
  "金狮子语音替换",
  "猎人据点灯光增强",
  "霜刃太刀外观包",
  "星辉大厅换装合集",
  "工坊机巧锤纹理包",
  "随从礼帽皮肤集",
];

const featuredMetadata = [
  { author: "NexusUser123", versionLabel: "v2.1.4" },
  { author: "NexusUser123", versionLabel: "v1.0.0" },
];

function parseArgs(argv) {
  const options = {
    count: 10,
    output: defaultOutputPath,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === "--") {
      continue;
    }

    if (arg === "--count") {
      const value = argv[index + 1];
      if (value == null) {
        throw new Error("Missing value for --count");
      }
      options.count = validateCount(value);
      index += 1;
      continue;
    }

    if (arg === "--output") {
      const value = argv[index + 1];
      if (value == null) {
        throw new Error("Missing value for --output");
      }
      options.output = resolve(repoRoot, value);
      index += 1;
      continue;
    }

    if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    }

    throw new Error(`Unknown argument: ${arg}`);
  }

  return options;
}

function validateCount(value) {
  if (!/^\d+$/.test(value)) {
    throw new Error(`Invalid count "${value}". Count must be an integer between 1 and 999.`);
  }

  const count = Number(value);
  if (!Number.isInteger(count) || count < 1 || count > 999) {
    throw new Error(`Invalid count "${value}". Count must be an integer between 1 and 999.`);
  }

  return count;
}

function slugify(input) {
  return input
    .toLowerCase()
    .replace(/[^a-z0-9\u4e00-\u9fa5]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function makeSizeLabel(index) {
  if (index % 9 === 1) {
    return "默认封面";
  }

  const size = 1.5 + ((index * 7) % 48) / 10;
  return `${size.toFixed(1)} MB`;
}

function makeName(index) {
  const baseName = seedNames[index % seedNames.length];
  const series = Math.floor(index / seedNames.length) + 1;
  return series === 1 ? baseName : `${baseName} ${series}号变体`;
}

function buildModLibraryItems(count) {
  return Array.from({ length: count }, (_, index) => {
    const itemIndex = index + 1;
    const name = makeName(index);
    const [posterFrom, posterTo] = gradientPairs[index % gradientPairs.length];
    const categoryLabels = categoryCycle[index % categoryCycle.length];
    const status = statusCycle[index % statusCycle.length];
    const metadata = featuredMetadata[index];

    return {
      id: `mod-${String(itemIndex).padStart(3, "0")}-${slugify(name)}`,
      name,
      ...metadata,
      sizeLabel: makeSizeLabel(itemIndex),
      status,
      categoryLabels,
      posterFrom,
      posterTo,
    };
  });
}

function formatItems(items) {
  return items
    .map((item) => {
      const metadataLines = [
        item.author == null ? null : `    author: ${JSON.stringify(item.author)},`,
        item.versionLabel == null ? null : `    versionLabel: ${JSON.stringify(item.versionLabel)},`,
      ]
        .filter(Boolean)
        .join("\n");

      return `  {
    id: ${JSON.stringify(item.id)},
    name: ${JSON.stringify(item.name)},
${metadataLines.length === 0 ? "" : `${metadataLines}\n`}    sizeLabel: ${JSON.stringify(item.sizeLabel)},
    status: ${JSON.stringify(item.status)},
    categoryLabels: [${item.categoryLabels
      .map((label) => `{ name: ${JSON.stringify(label)} }`)
      .join(", ")}],
    posterFrom: ${JSON.stringify(item.posterFrom)},
    posterTo: ${JSON.stringify(item.posterTo)},
  },`;
    })
    .join("\n");
}

function renderFileContent(items) {
  return `// Mod 库展示层数据。
// 当前文件由 scripts/generate-mod-library-mock-data.mjs 生成。
// 当前生成数量：${items.length}
// 现阶段使用本地 mock 数据还原设计稿，后续由 Mod 仓储或视图模型提供真实数据。
// 业务规则（安装、冲突、依赖判定）不在此处推断，仅承载展示字段。
import type { ModLibraryItem } from "./modLibraryTypes";
export type { ModInstallStatus, ModLibraryItem } from "./modLibraryTypes";

export const modLibraryItems: ModLibraryItem[] = [
${formatItems(items)}
];

// 快捷操作面板的动作项。
// 仅承载展示语义，点击行为由页面层透传，不在此处实现业务。
export type CompactActionVariant = "primary" | "neutral" | "success" | "warning" | "danger" | "info";

export type CompactAction = {
  id: string;
  label: string;
  variant: CompactActionVariant;
};

export const compactActions: CompactAction[] = [
  { id: "add", label: "导入 Mod", variant: "primary" },
  { id: "add-revision", label: "导入新版本", variant: "info" },
  { id: "select-all", label: "选择本页", variant: "neutral" },
  { id: "invert", label: "反选本页", variant: "neutral" },
  { id: "refresh", label: "刷新", variant: "neutral" },
  { id: "preview-plan", label: "预览安装计划", variant: "info" },
  { id: "install", label: "安装选中 MOD", variant: "success" },
  { id: "reinstall", label: "重装选中 MOD", variant: "info" },
  { id: "uninstall", label: "卸载选中 MOD", variant: "danger" },
];

export const libraryFilterChips = ["全部", "已安装", "已禁用", "存在冲突", "外观", "武器", "语音"] as const;
`;
}

function printHelp() {
  console.log(`Usage: node scripts/generate-mod-library-mock-data.mjs [--count <1-999>] [--output <path>]

Options:
  --count   Number of mod library mock items to generate. Default: 10
  --output  Output file path. Default: src/features/mods/modsLibraryData.ts`);
}

function main() {
  try {
    const options = parseArgs(process.argv.slice(2));
    const items = buildModLibraryItems(options.count);
    const content = renderFileContent(items);

    mkdirSync(dirname(options.output), { recursive: true });
    writeFileSync(options.output, content, "utf8");

    console.log(`Generated ${items.length} mod library mock items at ${options.output}`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}

main();
