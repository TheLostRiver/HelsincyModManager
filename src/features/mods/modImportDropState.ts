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
//
// ## 清单是长活的，不是一次性的
//
// 首版把清单做成了「一批跑完为止」的模态：导入期间关不掉、也不能再拖新的。玩家拖 30 个包
// 就得干等，整个 HMM 不可用。改成**行有生命周期、队列可追加**之后：浮层随时可关（任务在
// 后台继续）、随时可重开查看、导入中还能继续拖新的进来追加。
//
// 于是「一次运行」不再是独立对象——**行本身就是唯一事实来源**。原先的 `DropImportRun`
// 快照被删掉了：它假定队列在开跑那一刻就定死，而这与「随时可追加」直接冲突。

/** 后端 `preview_dropped_mod_archives` 的逐条结果。 */
export type DroppedArchivePreview = {
  archivePath: string;
  fileName: string;
  /** 文件字节数；`null` = 读不到。**不影响可导入性**，只是给玩家核对用。 */
  sizeBytes: number | null;
  /** `null` = 可导入；否则是与导入失败同一套的语义码。 */
  errorCode: string | null;
  /**
   * 内容层警示码。**与 `errorCode` 不是一类**——那个是「读不了」，这个是
   * 「读得了，但看起来装不出东西」。只警示，玩家可以覆盖。
   */
  warningCode: string | null;
};

/** 内容层警示的档位。目前只有一档，留成联合类型是为了新增时有穷尽性检查。 */
export type DropRowWarningKind = "no-game-content";

const warningKindByCode: Readonly<Record<string, DropRowWarningKind>> = {
  mod_import_archive_no_game_content: "no-game-content",
};

/**
 * 预检结论，三档不是两档。
 *
 * - `importable`：能导，默认勾选
 * - `warned`：能导，但看起来装不出东西。默认**不**勾选，**但必须能勾回来**
 * - `blocked`：链路物理上读不了。不勾选，且**不能**勾
 *
 * `warned` 与 `blocked` 的区别是这一整档存在的理由：包级否决是错的，我们的判定会错，
 * 而错的代价是玩家眼睁睁看着一个好包装不进来（#350 / #354 那一整轮的教训）。
 */
export type DropRowStatus = "importable" | "warned" | "blocked";

/**
 * 行在**执行**这条轴上的位置，与 `status`（预检结论）正交。
 *
 * 两条轴不能合并：`warned` 说的是「我们对这个包的判断」，`phase` 说的是「它跑到哪了」，
 * 一个 `warned` 的行被玩家勾上之后照样会走完 queued → running → succeeded。
 */
export type DropRowPhase = "pending" | "queued" | "running" | "succeeded" | "failed";

export type DropRow = {
  archivePath: string;
  fileName: string;
  sizeBytes: number | null;
  status: DropRowStatus;
  /** 只有 `blocked` 行才有；复用导入失败的档位，不另造词汇。 */
  messageKind: ModImportFailedMessageKind | null;
  /** 只有 `warned` 行才有。 */
  warningKind: DropRowWarningKind | null;
  /** 只对 `pending` 行有意义。 */
  selected: boolean;
  phase: DropRowPhase;
};

/**
 * 清单状态。
 *
 * `checking` 与既有行**共存**：导入进行中再拖一批进来，旧行照常显示进度，新一批在后端
 * 预检。做成互斥的联合类型会逼出「预检期间清单消失」这种更差的表现。
 */
export type DropListState = {
  rows: DropRow[];
  /** 正在后端预检的文件数。 */
  checking: number;
};

export const emptyDropListState: DropListState = { rows: [], checking: 0 };

/** 能不能勾。`blocked` 不能——硬事实；已提交的行也不能——它已经不归玩家管了。 */
export function isDropRowSelectable(row: DropRow): boolean {
  return row.status !== "blocked" && row.phase === "pending";
}

/** 还等着玩家决定的行。 */
export function pendingDropRows(rows: readonly DropRow[]): DropRow[] {
  return rows.filter((row) => row.phase === "pending");
}

function rowFromPreview(preview: DroppedArchivePreview): DropRow {
  if (preview.errorCode !== null) {
    return {
      archivePath: preview.archivePath,
      fileName: preview.fileName,
      sizeBytes: preview.sizeBytes,
      status: "blocked",
      messageKind: failedMessageKindFrom(preview.errorCode),
      warningKind: null,
      selected: false,
      phase: "pending",
    };
  }
  // 认不出的警示码**不当成警示**：宁可什么都不说，也不要摆一个空提示语，
  // 更不能因为后端多发了一个我们还不认识的码就把行默认取消勾选。
  const warningKind =
    preview.warningCode === null ? null : warningKindByCode[preview.warningCode] ?? null;
  return {
    archivePath: preview.archivePath,
    fileName: preview.fileName,
    sizeBytes: preview.sizeBytes,
    status: warningKind === null ? "importable" : "warned",
    messageKind: null,
    warningKind,
    // 警示档默认不勾选——但 toggle 与全选都允许把它勾回来。
    selected: warningKind === null,
    phase: "pending",
  };
}

/** 后端预检结果 → 清单行。 */
export function dropRowsFromPreviews(previews: readonly DroppedArchivePreview[]): DropRow[] {
  return previews.map(rowFromPreview);
}

/**
 * 把新一批预检结果并入既有清单。
 *
 * 同路径的处理分两种，因为它们是两回事：
 * - 还没跑完（`pending` / `queued` / `running`）→ **跳过**。重复入队会对同一个文件起两个
 *   导入任务，第二个必然失败，玩家看到一条莫名其妙的失败。
 * - 已经跑完（`succeeded` / `failed`）→ **重置成 pending**，让玩家能重试。清单现在是长活的，
 *   不重置的话一个失败过的包在关掉浮层之前永远没法再试。
 */
export function mergeDropRows(
  existing: readonly DropRow[],
  previews: readonly DroppedArchivePreview[],
): DropRow[] {
  const merged = [...existing];
  const indexByPath = new Map(merged.map((row, index) => [row.archivePath, index]));

  for (const preview of previews) {
    const index = indexByPath.get(preview.archivePath);
    if (index === undefined) {
      indexByPath.set(preview.archivePath, merged.length);
      merged.push(rowFromPreview(preview));
      continue;
    }
    const current = merged[index];
    if (current.phase === "succeeded" || current.phase === "failed") {
      merged[index] = rowFromPreview(preview);
    }
  }

  return merged;
}

/** 逐行切换勾选。不可勾的行**不响应**——它不是「默认不选」，是「不能选」。 */
export function toggleDropRow(rows: readonly DropRow[], archivePath: string): DropRow[] {
  return rows.map((row) =>
    row.archivePath === archivePath && isDropRowSelectable(row)
      ? { ...row, selected: !row.selected }
      : row,
  );
}

/** 全选 / 全不选。只作用于还能勾的行。 */
export function setAllDropRowsSelected(rows: readonly DropRow[], selected: boolean): DropRow[] {
  // 「全选」把警示档也勾上：玩家明确要求了全部，而警示档本来就允许覆盖。
  return rows.map((row) => (isDropRowSelectable(row) ? { ...row, selected } : row));
}

export function selectedDropRows(rows: readonly DropRow[]): DropRow[] {
  return rows.filter((row) => isDropRowSelectable(row) && row.selected);
}

/** 还能勾的行数（「已选 x / y」里的 y）。 */
export function selectableDropRowCount(rows: readonly DropRow[]): number {
  return rows.filter(isDropRowSelectable).length;
}

/**
 * 「全选」复选框的三态。
 *
 * 没有任何可勾行时是 `none`（而不是 `all`）——否则一个整批都读不了的拖拽会显示成
 * 「已全选」，然后确认按钮却是灰的，自相矛盾。
 */
export function dropSelectAllState(rows: readonly DropRow[]): "none" | "some" | "all" {
  const selectable = rows.filter(isDropRowSelectable);
  if (selectable.length === 0) return "none";
  const selected = selectable.filter((row) => row.selected).length;
  if (selected === 0) return "none";
  return selected === selectable.length ? "all" : "some";
}

/** 确认按钮可用性：**至少选中一个**才可导入。 */
export function canStartDropImport(rows: readonly DropRow[]): boolean {
  return selectedDropRows(rows).length > 0;
}

/**
 * 玩家按下确认：选中的行转成 `queued`，并给出要入队的路径。
 *
 * 返回新的行数组与待入队路径，而不是直接改队列——入队是副作用，留给调用方，
 * 这一层保持纯函数，才能逐条断言。
 */
export function confirmDropSelection(rows: readonly DropRow[]): {
  rows: DropRow[];
  queued: string[];
} {
  const queued: string[] = [];
  const next = rows.map((row) => {
    if (!isDropRowSelectable(row) || !row.selected) return row;
    queued.push(row.archivePath);
    return { ...row, phase: "queued" as const, selected: false };
  });
  return { rows: next, queued };
}

export function markDropRowPhase(
  rows: readonly DropRow[],
  archivePath: string,
  phase: DropRowPhase,
): DropRow[] {
  return rows.map((row) => (row.archivePath === archivePath ? { ...row, phase } : row));
}

/**
 * 停止后续：把还没起步的 `queued` 行退回 `pending`。
 *
 * **不碰 `running` 那个**——导入任务一旦起步就由后端的任务机制管，这里能保证的只有
 * 「不再往下起」。退回 `pending` 而不是记成失败：它根本没跑过，记成失败是撒谎，
 * 而且退回之后玩家还能再确认一次。
 */
export function cancelQueuedDropRows(rows: readonly DropRow[]): DropRow[] {
  return rows.map((row) =>
    row.phase === "queued" ? { ...row, phase: "pending" as const, selected: true } : row,
  );
}

/** 清掉已经跑完的行，让长活的清单不至于无限变长。 */
export function clearFinishedDropRows(rows: readonly DropRow[]): DropRow[] {
  return rows.filter((row) => row.phase !== "succeeded" && row.phase !== "failed");
}

export type DropQueueSummary = {
  pending: number;
  queued: number;
  running: number;
  succeeded: number;
  failed: number;
  /** 本轮已经提交过的总数（不含还没确认的 pending）。 */
  submitted: number;
  /** 还有东西在跑或在排队。 */
  active: boolean;
};

export function dropQueueSummary(rows: readonly DropRow[]): DropQueueSummary {
  const count = (phase: DropRowPhase) => rows.filter((row) => row.phase === phase).length;
  const queued = count("queued");
  const running = count("running");
  const succeeded = count("succeeded");
  const failed = count("failed");
  return {
    pending: count("pending"),
    queued,
    running,
    succeeded,
    failed,
    submitted: queued + running + succeeded + failed,
    active: queued + running > 0,
  };
}

/**
 * 行的提示语。
 *
 * `blocked` 复用导入失败的三语文案（不另造词汇）；`warned` 用内容层自己的一句，
 * 而那句必须说明「仍然可以导入」——否则玩家会以为它和读不了的行是一回事。
 */
export function getDropRowNote(row: DropRow, copy: ModImportCopy): string | null {
  if (row.messageKind !== null) return getModImportFailedMessage(row.messageKind, copy);
  if (row.warningKind === "no-game-content") return copy.drop.warnNoGameContent;
  return null;
}

/**
 * 一次拖拽能接收的文件数上限。
 *
 * 超了就**整批拒绝并说清楚数量**，不截断——截断等于悄悄丢掉玩家拖进来的东西，
 * 而他多半不会去数清单有几行。上限本身是为了不让预检把界面拖住：
 * 每个文件都要真的打开归档读头。
 */
export const MAX_DROPPED_ARCHIVES = 100;

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
