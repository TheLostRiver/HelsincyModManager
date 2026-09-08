import {
  advanceDropImportRun,
  startDropImportRun,
  stopDropImportRun,
  type DropImportOutcome,
  type DropImportRun,
  type DropRow,
  // 带 `.ts` 后缀：本模块要能被 node --test 直接跑，值导入需要显式扩展名。
} from "./modImportDropState.ts";
import { nextModImportTaskStateFromProgress, type ModImportTaskState } from "./modImportTaskState.ts";
import type { TaskProgressEventDto } from "./modImportTypes";

// 拖拽批量导入的执行引擎（T22 / #366）。
//
// 从组件里拆出来是为了**能真的断言行为**：仓库的前端测试大多只能正则读源码，
// 而这里恰好是全部难点所在——进度事件比 taskId 先到、终态识别、串行推进、中途停止。
// 拆出来之后这些都能用假依赖逐条跑，不需要 DOM。

type Tracker = {
  state: ModImportTaskState;
  settled: boolean;
  resolve: (outcome: DropImportOutcome) => void;
};

/** `start_import_mod_task` 返回的形状里，这里真正用到的部分。 */
export type StartedImportTask = { kind: string; status: string; taskId: string };

function isTerminal(state: ModImportTaskState) {
  return state.status === "completed" || state.status === "cancelled" || state.status === "failed";
}

/**
 * 把任务进度事件翻译成「这一个导入成功了还是失败了」。
 *
 * **必须先缓存再认领**：`start_import_mod_task` 返回 taskId 之前进度事件就可能到，
 * 那时没人知道该把它算给谁。丢掉它的话，一个瞬间就跑完的导入会永远等不到终态。
 */
export class ModImportTaskWatcher {
  private readonly trackers = new Map<string, Tracker>();
  private readonly buffered = new Map<string, TaskProgressEventDto[]>();
  private startPending = false;

  /** 起任务之前调用：从这一刻起，认不出归属的事件先存着。 */
  beginStart(): void {
    this.startPending = true;
    this.buffered.clear();
  }

  /** 起任务的结果已知（无论成败）：不再缓存。 */
  endStart(): void {
    this.startPending = false;
    this.buffered.clear();
  }

  handleProgress(payload: TaskProgressEventDto): void {
    if (payload.kind !== "mod_import") return;
    if (this.trackers.has(payload.taskId)) {
      this.applyToTracker(payload);
      return;
    }
    if (!this.startPending) return;
    const queued = this.buffered.get(payload.taskId) ?? [];
    queued.push(payload);
    this.buffered.set(payload.taskId, queued);
  }

  /** 认领一个 taskId，返回它的终态。会先补放缓存下来的事件。 */
  watch(taskId: string): Promise<DropImportOutcome> {
    const outcome = new Promise<DropImportOutcome>((resolve) => {
      this.trackers.set(taskId, {
        state: { status: "running", taskId, phase: "mod_import.queued" },
        settled: false,
        resolve,
      });
    });
    for (const payload of this.buffered.get(taskId) ?? []) {
      this.applyToTracker(payload);
    }
    return outcome;
  }

  private applyToTracker(payload: TaskProgressEventDto): void {
    const tracker = this.trackers.get(payload.taskId);
    if (!tracker || tracker.settled) return;
    tracker.state = nextModImportTaskStateFromProgress(tracker.state, payload);
    if (!isTerminal(tracker.state)) return;
    tracker.settled = true;
    this.trackers.delete(payload.taskId);
    // 取消也算这一条没导进去。**不把取消说成成功**——库里确实没多出这个 Mod。
    tracker.resolve(tracker.state.status === "completed" ? "succeeded" : "failed");
  }
}

export type DropImportBatchDeps = {
  watcher: ModImportTaskWatcher;
  startImport: (archivePath: string) => Promise<StartedImportTask>;
  /** 每推进一步回调一次，供界面显示进度。 */
  onRunChange: (run: DropImportRun) => void;
  /** 每次起下一个之前问一次；true = 不再往下起。 */
  shouldStop: () => boolean;
};

/** 起一个导入并等它的终态。起不来（抛错或状态不对）直接算这一条失败。 */
async function runOne(
  archivePath: string,
  { watcher, startImport }: Pick<DropImportBatchDeps, "watcher" | "startImport">,
): Promise<DropImportOutcome> {
  watcher.beginStart();
  let task: StartedImportTask;
  try {
    task = await startImport(archivePath);
  } catch {
    watcher.endStart();
    return "failed";
  }
  if (task.kind !== "mod_import" || task.status !== "queued") {
    watcher.endStart();
    return "failed";
  }
  const outcome = watcher.watch(task.taskId);
  watcher.endStart();
  return outcome;
}

/**
 * 逐个**串行**导入选中的行。
 *
 * 串行而不是并发：并发会同时抢沙箱与 unrar 的进程级锁，收益不明而失败模式很难解释；
 * 串行还让「第几个 / 共几个」对玩家是准确的。
 */
export async function runDropImportBatch(
  rows: readonly DropRow[],
  deps: DropImportBatchDeps,
): Promise<DropImportRun> {
  let run = startDropImportRun(rows);
  deps.onRunChange(run);

  while (run.currentPath !== null) {
    const outcome = await runOne(run.currentPath, deps);
    run = advanceDropImportRun(run, outcome);
    if (deps.shouldStop()) run = stopDropImportRun(run);
    deps.onRunChange(run);
  }

  return run;
}
