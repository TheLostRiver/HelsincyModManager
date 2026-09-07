import {
  failedMessageKindFrom,
  getModImportFailedMessage,
  type ModImportFailedMessageKind,
  // 带 `.ts` 后缀：本模块要能被 node --test 直接跑，而它对**值导入**需要显式扩展名
  // （类型导入会被擦除，所以下面那行不需要）。仓库里已有同样的先例。
} from "./modImportTaskState.ts";
import type { ModImportCopy } from "./modImportCopy";

// 拖拽导入的待导入清单模型（T22 / #366）。
//
// **落点是确认清单，不是导入动作。** 拖进来只产生一份可逐条勾选的清单；
// 玩家确认之后才真的导入。这一层只有纯函数与状态，不碰 Tauri、不碰 DOM，
// 所以能逐条行为断言，而不是靠正则读源码。

/** 后端 `preview_dropped_mod_archives` 的逐条结果。 */
export type DroppedArchivePreview = {
  archivePath: string;
  fileName: string;
  /** `null` = 可导入；否则是与导入失败同一套的语义码。 */
  errorCode: string | null;
};

export type DropRowStatus = "importable" | "blocked";

export type DropRow = {
  archivePath: string;
  fileName: string;
  status: DropRowStatus;
  /** 只有 `blocked` 行才有；复用导入失败的档位，不另造词汇。 */
  messageKind: ModImportFailedMessageKind | null;
  selected: boolean;
};

export type DropListState =
  | { status: "idle" }
  /** 文件已拖进来、后端还在逐个预检。清单必须能表达这个中间态。 */
  | { status: "checking"; total: number }
  | { status: "ready"; rows: DropRow[] }
  | { status: "importing"; rows: DropRow[]; startedPaths: string[] };

/**
 * 后端预检结果 → 清单行。
 *
 * **不可导入的行默认不勾选，而且不允许勾上**：容器层那几档是硬事实
 * ——链路物理上读不了，给个能点的勾选框只是骗人。
 */
export function dropRowsFromPreviews(previews: readonly DroppedArchivePreview[]): DropRow[] {
  return previews.map((preview) => {
    const importable = preview.errorCode === null;
    return {
      archivePath: preview.archivePath,
      fileName: preview.fileName,
      status: importable ? "importable" : "blocked",
      messageKind: importable ? null : failedMessageKindFrom(preview.errorCode),
      selected: importable,
    };
  });
}

/** 逐行切换勾选。`blocked` 行**不响应**——它不是「默认不选」，是「不能选」。 */
export function toggleDropRow(rows: readonly DropRow[], archivePath: string): DropRow[] {
  return rows.map((row) =>
    row.archivePath === archivePath && row.status === "importable"
      ? { ...row, selected: !row.selected }
      : row,
  );
}

/** 全选 / 全不选，只作用于可导入的行。 */
export function setAllDropRowsSelected(rows: readonly DropRow[], selected: boolean): DropRow[] {
  return rows.map((row) => (row.status === "importable" ? { ...row, selected } : row));
}

export function selectedDropRows(rows: readonly DropRow[]): DropRow[] {
  return rows.filter((row) => row.status === "importable" && row.selected);
}

export function importableDropRowCount(rows: readonly DropRow[]): number {
  return rows.filter((row) => row.status === "importable").length;
}

/**
 * 「全选」复选框的三态。
 *
 * 没有任何可导入行时是 `none`（而不是 `all`）——否则一个整批都读不了的拖拽会显示成
 * 「已全选」，然后确认按钮却是灰的，自相矛盾。
 */
export function dropSelectAllState(rows: readonly DropRow[]): "none" | "some" | "all" {
  const importable = rows.filter((row) => row.status === "importable");
  if (importable.length === 0) return "none";
  const selected = importable.filter((row) => row.selected).length;
  if (selected === 0) return "none";
  return selected === importable.length ? "all" : "some";
}

/** 确认按钮可用性：**至少选中一个**才可导入。 */
export function canStartDropImport(rows: readonly DropRow[]): boolean {
  return selectedDropRows(rows).length > 0;
}

/** 被挡住的行的提示语，复用导入失败的三语文案。 */
export function getDropRowBlockedMessage(row: DropRow, copy: ModImportCopy): string | null {
  return row.messageKind === null ? null : getModImportFailedMessage(row.messageKind, copy);
}

/**
 * 只保留能被导入链路处理的路径。
 *
 * Tauri 的拖放事件会把**目录**也一起给过来，而导入要的是压缩包文件。
 * 目录在后端预检里会落到 `retry-hint`，所以不必在这里预先剔除
 * ——**判定只有一处**，前端不重复实现一份「什么算压缩包」。
 */
export function dedupeDroppedPaths(paths: readonly string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const path of paths) {
    // 同一次拖拽里重复的路径只留一条：否则会对同一个文件起两个导入任务，
    // 第二个必然因为目标已存在而失败，玩家看到一条莫名其妙的失败。
    if (path.length > 0 && !seen.has(path)) {
      seen.add(path);
      result.push(path);
    }
  }
  return result;
}

// ---- 确认之后的批量执行 ----
//
// 逐个**串行**跑：并发导入会同时抢沙箱与 unrar 的进程级锁（rar 不能并发调用），
// 收益不明而失败模式很难解释。串行还让「第几个 / 共几个」这件事对玩家是准确的。

export type DropImportOutcome = "succeeded" | "failed";

export type DropImportRun = {
  /** 还没开始的路径，按玩家看到的顺序。 */
  queue: string[];
  /** 正在跑的那个；`null` = 跑完了。 */
  currentPath: string | null;
  results: Record<string, DropImportOutcome>;
  total: number;
};

export function startDropImportRun(rows: readonly DropRow[]): DropImportRun {
  const paths = selectedDropRows(rows).map((row) => row.archivePath);
  const [first, ...rest] = paths;
  return {
    queue: rest,
    currentPath: first ?? null,
    results: {},
    total: paths.length,
  };
}

/** 当前这个跑完了（成功或失败），推进到下一个。 */
export function advanceDropImportRun(run: DropImportRun, outcome: DropImportOutcome): DropImportRun {
  if (run.currentPath === null) return run;
  const [next, ...rest] = run.queue;
  return {
    queue: rest,
    currentPath: next ?? null,
    results: { ...run.results, [run.currentPath]: outcome },
    total: run.total,
  };
}

/**
 * 停止后续排队项。**不中断正在跑的那个**——导入任务一旦起步就由后端的任务机制管，
 * 这里能保证的只有「不再往下起」。
 *
 * `currentPath` **也要清掉**，不能只清队列：调用点的顺序是「跑完 → advance → 停止」，
 * 而 advance 已经把下一个提升成了 `currentPath`。只清 `queue` 会让那个刚被提升、
 * 还没起步的又跑掉——恰好多跑一个，正是玩家按停止想避免的。
 *
 * `total` 保持原样：摘要里 `finished < total` 正是「后面那些没跑」的如实体现，
 * 而不是把它们记成失败。
 */
export function stopDropImportRun(run: DropImportRun): DropImportRun {
  return { ...run, queue: [], currentPath: null };
}

export function dropImportRunSummary(run: DropImportRun) {
  const outcomes = Object.values(run.results);
  const succeeded = outcomes.filter((outcome) => outcome === "succeeded").length;
  return {
    succeeded,
    failed: outcomes.length - succeeded,
    finished: outcomes.length,
    total: run.total,
    // 跑完 = 没有正在跑的了。**不看 finished === total**：那样一个都没选时会
    // 立刻算成「跑完」，而实际上根本没开始。
    done: run.currentPath === null,
  };
}
