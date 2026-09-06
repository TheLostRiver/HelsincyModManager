import type { PackageContentEntry } from "./packageContentsTypes";

/*
 * 包内容树模型（`#354` 切片 D4）。
 *
 * 后端给的是**扁平**清单（`get_mod_package_contents` 模块头写明了理由：扁平更好序列化、
 * 更好 diff，实测最大的包 7340 文件、深度 10，嵌套结构在这个量级上传输与比对都更贵）。
 * 建树因此是前端的职责，这个模块就是那一层——纯函数、不碰 React、可单独喂输入断言输出。
 *
 * 树骨架按 `packageFileId` 切分：它是**沙箱根**相对路径，代表包里真实的目录结构。
 * 不用 `targetPath`——那是内容根相对路径，内容根之外的文件根本没有它（为 `null`），
 * 按它建树会让「包里有什么」这个问题答不完整，而玩家挑内容根的前提正是看得见整包。
 */

/** 目录节点聚合的子孙统计。折叠状态下也要能一眼看出这个目录里有什么。 */
export type PackageTreeStats = {
  fileCount: number;
  /** 能落进本 game adapter 允许安装根的文件数。 */
  installableCount: number;
  /** 命中本游戏「绝不安装」清单的文件数。 */
  rejectedByGameCount: number;
  /** 玩家勾掉的文件数。 */
  excludedByPlayerCount: number;
  totalSizeBytes: number;
};

export type PackageTreeDirectoryNode = {
  kind: "directory";
  /** 沙箱根相对路径，同时是展开状态与级联操作的键。 */
  path: string;
  name: string;
  depth: number;
  children: PackageTreeNode[];
  stats: PackageTreeStats;
};

export type PackageTreeFileNode = {
  kind: "file";
  /** 等于 `entry.packageFileId`。 */
  path: string;
  name: string;
  depth: number;
  entry: PackageContentEntry;
};

export type PackageTreeNode = PackageTreeDirectoryNode | PackageTreeFileNode;

/** 扁平化之后的一行，字段够直接渲染成 `treeitem`（含 a11y 所需的位置信息）。 */
export type PackageTreeRow = {
  node: PackageTreeNode;
  /** 从 1 起算，对应 `aria-level`。 */
  level: number;
  /** 同级兄弟总数，对应 `aria-setsize`。 */
  setSize: number;
  /** 从 1 起算的同级序号，对应 `aria-posinset`。 */
  posInSet: number;
  /** 目录才有；文件为 `undefined`，渲染时不应输出 `aria-expanded`。 */
  isExpanded?: boolean;
};

const EMPTY_STATS: PackageTreeStats = {
  fileCount: 0,
  installableCount: 0,
  rejectedByGameCount: 0,
  excludedByPlayerCount: 0,
  totalSizeBytes: 0,
};

type MutableDirectory = {
  name: string;
  path: string;
  directories: Map<string, MutableDirectory>;
  files: PackageContentEntry[];
};

function createDirectory(name: string, path: string): MutableDirectory {
  return { name, path, directories: new Map(), files: [] };
}

/**
 * 切分 `packageFileId`。
 *
 * 后端 `sandbox_install_relative_path` 已经把分隔符规范成 `/` 并拒绝了空段与 `..`，
 * 所以这里只做切分，不再发明格式校验——凭空加的校验会在第一个反例上把正常包判成坏包。
 * 空段仍然过滤掉，是因为它对建树无意义（不是安全判断，只是不想造出没有名字的层）。
 */
function splitPackageFileId(packageFileId: string): string[] {
  return packageFileId.split("/").filter((segment) => segment.length > 0);
}

/**
 * 扁平清单 → 树。
 *
 * 复杂度 O(条目数 × 深度)：每条只沿自己的路径走一遍。7340 文件 × 深度 10 是一次性的
 * 几万次 Map 操作，远低于让它变成渲染瓶颈的量级。
 */
export function buildPackageContentTree(entries: readonly PackageContentEntry[]): PackageTreeNode[] {
  const root = createDirectory("", "");

  for (const entry of entries) {
    const segments = splitPackageFileId(entry.packageFileId);
    if (segments.length === 0) {
      continue;
    }

    let cursor = root;
    for (let index = 0; index < segments.length - 1; index += 1) {
      const segment = segments[index];
      const path = cursor.path === "" ? segment : `${cursor.path}/${segment}`;
      let next = cursor.directories.get(segment);
      if (!next) {
        next = createDirectory(segment, path);
        cursor.directories.set(segment, next);
      }
      cursor = next;
    }
    cursor.files.push(entry);
  }

  return materializeChildren(root, 0).nodes;
}

/**
 * 递归物化一层，并把子孙统计沿途累加上来。
 *
 * 同层的排序是**目录在前、文件在后**，各自按名字的本地化顺序。后端按完整路径的字典序排过，
 * 但那份顺序里目录与文件是混的（`a.txt` 会排在 `a/` 之后或之前取决于字符），直接用会让
 * 界面上的同一层看起来是乱的。
 */
function materializeChildren(
  directory: MutableDirectory,
  depth: number,
): { nodes: PackageTreeNode[]; stats: PackageTreeStats } {
  const directoryNodes: PackageTreeDirectoryNode[] = [];
  let stats = EMPTY_STATS;

  for (const child of directory.directories.values()) {
    const materialized = materializeChildren(child, depth + 1);
    directoryNodes.push({
      kind: "directory",
      path: child.path,
      name: child.name,
      depth,
      children: materialized.nodes,
      stats: materialized.stats,
    });
    stats = mergeStats(stats, materialized.stats);
  }

  const fileNodes: PackageTreeFileNode[] = directory.files.map((entry) => {
    const segments = splitPackageFileId(entry.packageFileId);
    return {
      kind: "file" as const,
      path: entry.packageFileId,
      name: segments[segments.length - 1] ?? entry.packageFileId,
      depth,
      entry,
    };
  });

  for (const file of fileNodes) {
    stats = mergeStats(stats, statsOfEntry(file.entry));
  }

  directoryNodes.sort((left, right) => compareNames(left.name, right.name));
  fileNodes.sort((left, right) => compareNames(left.name, right.name));

  return { nodes: [...directoryNodes, ...fileNodes], stats };
}

function compareNames(left: string, right: string): number {
  return left.localeCompare(right, undefined, { numeric: true, sensitivity: "base" });
}

function statsOfEntry(entry: PackageContentEntry): PackageTreeStats {
  return {
    fileCount: 1,
    installableCount: entry.installable ? 1 : 0,
    rejectedByGameCount: entry.rejectedByGame ? 1 : 0,
    excludedByPlayerCount: entry.excludedByPlayer ? 1 : 0,
    totalSizeBytes: entry.sizeBytes,
  };
}

function mergeStats(left: PackageTreeStats, right: PackageTreeStats): PackageTreeStats {
  return {
    fileCount: left.fileCount + right.fileCount,
    installableCount: left.installableCount + right.installableCount,
    rejectedByGameCount: left.rejectedByGameCount + right.rejectedByGameCount,
    excludedByPlayerCount: left.excludedByPlayerCount + right.excludedByPlayerCount,
    totalSizeBytes: left.totalSizeBytes + right.totalSizeBytes,
  };
}

/**
 * 打开页面时该展开哪些目录。
 *
 * **默认折叠是给极端包定的规矩，不该套到常见包上。** 实测外观包只有 20–130 文件，全展开
 * 也就几十行；让玩家为了看清一个 28 文件的包去逐层点开，是拿最大包的约束惩罚所有人。
 * 滚动是廉价的，点击是昂贵的——能一次看全就别让他点。
 *
 * 两段策略：
 *
 * 1. **单目录链无条件展开**。真实语料里绝大多数包在 `nativePC` 之上套着一到多层包装目录
 *    （`#284` 就是为这个改的内容根解析）。这些层没有信息量，不展开就只看见一个孤零零的
 *    `大剑/` 等着点。它们不计入预算。
 * 2. **再逐层向下展开，直到下一层会超出行预算**。整层整层地放，保证同一深度的兄弟要么
 *    都展开、要么都不展开——半开半闭的树看起来就是乱的。7340 文件的包会在很浅的层停下，
 *    28 文件的包则一路展到底。
 */
export function resolveInitialExpandedPaths(
  nodes: readonly PackageTreeNode[],
  options: { rowBudget?: number } = {},
): Set<string> {
  const budget = options.rowBudget ?? DEFAULT_AUTO_EXPAND_ROW_BUDGET;
  const expanded = new Set<string>();

  // 1. 单链：没有信息量的包装层，不计预算。
  let current = nodes;
  while (current.length === 1 && current[0].kind === "directory") {
    const directory = current[0];
    expanded.add(directory.path);
    current = directory.children;
  }

  // 2. 逐层放，超预算就停在上一层。
  let frontier = directoriesOf(current);
  while (frontier.length > 0) {
    const candidate = new Set(expanded);
    for (const directory of frontier) {
      candidate.add(directory.path);
    }
    if (flattenVisibleRows(nodes, candidate).length > budget) {
      break;
    }

    for (const directory of frontier) {
      expanded.add(directory.path);
    }
    frontier = frontier.flatMap((directory) => directoriesOf(directory.children));
  }

  return expanded;
}

/**
 * 自动展开的可见行预算。
 *
 * 定在 300：实测外观包 20–130 文件，连目录行也远低于这个数，因此常见包一路展到底；
 * 7340 文件的包会在很浅的层停下，剩下的交给玩家自己点。
 */
const DEFAULT_AUTO_EXPAND_ROW_BUDGET = 300;

function directoriesOf(nodes: readonly PackageTreeNode[]): PackageTreeDirectoryNode[] {
  return nodes.filter((node): node is PackageTreeDirectoryNode => node.kind === "directory");
}

/**
 * 树 → 可见行。
 *
 * 只产出**当前可见**的行：折叠目录的子孙不进结果。渲染层的窗口化因此可以直接对这个数组
 * 切片，两端（默认折叠 + 可见行窗口化）用的是同一套机制。
 */
export function flattenVisibleRows(
  nodes: readonly PackageTreeNode[],
  expandedPaths: ReadonlySet<string>,
): PackageTreeRow[] {
  const rows: PackageTreeRow[] = [];

  const walk = (siblings: readonly PackageTreeNode[], level: number): void => {
    siblings.forEach((node, index) => {
      const isExpanded = node.kind === "directory" ? expandedPaths.has(node.path) : undefined;
      rows.push({
        node,
        level,
        setSize: siblings.length,
        posInSet: index + 1,
        isExpanded,
      });

      if (node.kind === "directory" && isExpanded) {
        walk(node.children, level + 1);
      }
    });
  };

  walk(nodes, 1);
  return rows;
}

/**
 * `path` → 节点 的索引。
 *
 * 勾选、展开这些操作拿到的都是路径（DOM 里只有字符串），每次再遍历一遍树是 O(n)——
 * 7340 个节点的包点一下就是一次全树扫描。建一次索引换 O(1) 查找。
 */
export function indexNodesByPath(nodes: readonly PackageTreeNode[]): Map<string, PackageTreeNode> {
  const index = new Map<string, PackageTreeNode>();

  const walk = (list: readonly PackageTreeNode[]): void => {
    for (const node of list) {
      index.set(node.path, node);
      if (node.kind === "directory") {
        walk(node.children);
      }
    }
  };

  walk(nodes);
  return index;
}

/** 整包统计：树根那一层的聚合，用于页头摘要。 */
export function summarizeTree(nodes: readonly PackageTreeNode[]): PackageTreeStats {
  return nodes.reduce((accumulated, node) => {
    return mergeStats(accumulated, node.kind === "directory" ? node.stats : statsOfEntry(node.entry));
  }, EMPTY_STATS);
}
