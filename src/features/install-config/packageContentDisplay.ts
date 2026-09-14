import type { PackageTreeNode } from "./packageContentTree";

export type PackageFileFilter = "all" | "excluded";

/** 仅投影显示结构；path、entry 和目录统计始终来自原树，级联选择使用原节点索引。 */
export function projectPackageContentTree(
  nodes: readonly PackageTreeNode[],
  options: { compact: boolean; query: string; filter: PackageFileFilter; excludedFiles: ReadonlySet<string> },
): PackageTreeNode[] {
  const query = options.query.trim().replaceAll("\\", "/").toLocaleLowerCase();
  const visit = (node: PackageTreeNode): PackageTreeNode | null => {
    if (node.kind === "file") {
      if (options.filter === "excluded" && !options.excludedFiles.has(node.path)) return null;
      return node.path.toLocaleLowerCase().includes(query) ? node : null;
    }
    let current = node;
    const names = [node.name];
    // 在筛选前判断单链，避免搜索隐藏兄弟后把有分支的目录误压缩。
    while (options.compact && current.children.length === 1 && current.children[0].kind === "directory") {
      current = current.children[0];
      names.push(current.name);
    }
    const children = current.children.map(visit).filter((child): child is PackageTreeNode => child !== null);
    return children.length > 0 ? { ...current, name: names.join(" / "), children } : null;
  };
  return nodes.map(visit).filter((node): node is PackageTreeNode => node !== null);
}

export function collectDirectoryPaths(nodes: readonly PackageTreeNode[]): Set<string> {
  const paths = new Set<string>();
  const visit = (node: PackageTreeNode) => {
    if (node.kind !== "directory") return;
    paths.add(node.path);
    node.children.forEach(visit);
  };
  nodes.forEach(visit);
  return paths;
}
