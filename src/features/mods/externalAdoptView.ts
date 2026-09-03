// 外部 MOD 接管（#286 adopt）的可用性投影。**纯逻辑，不含文案**——文案在
// `externalStateCopy`，这里只决定「能不能接管、不能的话为什么、能的话接几个」。
//
// 判据镜像后端的锁外前置拒绝（`hmm_runtime::external_mod_adopt`），目的不是替后端
// 做判定——后端在锁内还会以当下事实重算——而是让按钮在**必然被拒**时就别亮：
// 用户点了再看到一条错误码，不如一开始就看到「为什么现在不能接管」。
//
// 计数口径与后端 `derive_external_adopt_plan` 逐条对齐：
// - 可接管 = `matched` 且无占用（`claimedByModId` 缺席）；
// - `matched` 但被其他 MOD 占用 → 跳过（claimed）；
// - `changed` / `missing` → 跳过，只计数；
// - 任一 `unreadable` → 整次阻断（规则 3，拍板为阻断而非强确认）。
//
// `stale`（getter 重新 stat 与记录不一致）不是后端的前置拒绝，但后端锁内复核**必然**报
// stale——与其让用户点了再失败，不如直接要求重新检查。

import type { ExternalInstallStateSummary } from "./externalInstallStatusView";
import type { ExternalModStateDto } from "./externalStateApi";

/** 接管会写出的条目数与不会写出的各类计数，供确认弹窗如实陈述。 */
export type ExternalAdoptCounts = {
  claimable: number;
  skippedChanged: number;
  skippedMissing: number;
  skippedClaimed: number;
};

export type ExternalAdoptBlockedReason =
  /** 从未成功扫描过：接管消费的正是那份记录。 */
  | "no_summary"
  /** 比对集为空（unknown 态）：没有任何可比对的文件。 */
  | "unknown"
  /** 有读不到的文件：残缺事实上不建清单。 */
  | "unreadable"
  /** 结果可能过期：锁内复核必然拒绝，先重新检查。 */
  | "stale"
  /** 没有任何「一致且无主」的文件。 */
  | "nothing_to_adopt";

export type ExternalAdoptAvailability =
  | { status: "available"; counts: ExternalAdoptCounts }
  | { status: "blocked"; reason: ExternalAdoptBlockedReason };

export function externalAdoptCounts(summary: ExternalInstallStateSummary): ExternalAdoptCounts {
  const counts: ExternalAdoptCounts = {
    claimable: 0,
    skippedChanged: 0,
    skippedMissing: 0,
    skippedClaimed: 0,
  };
  for (const file of summary.files) {
    switch (file.state) {
      case "changed":
        counts.skippedChanged += 1;
        break;
      case "missing":
        counts.skippedMissing += 1;
        break;
      case "matched":
        if (file.claimedByModId !== undefined) {
          counts.skippedClaimed += 1;
        } else {
          counts.claimable += 1;
        }
        break;
      case "unreadable":
        // 调用方已整体拒绝；这里不归到任何一类，避免把「读不到」算成「已改动」。
        break;
    }
  }
  return counts;
}

export function projectExternalAdoptAvailability(
  state: ExternalModStateDto | null,
): ExternalAdoptAvailability {
  const summary = state?.summary ?? null;
  if (summary === null) {
    return { status: "blocked", reason: "no_summary" };
  }
  if (summary.files.length === 0) {
    return { status: "blocked", reason: "unknown" };
  }
  // 读不到排在过期之前：两者都要重新检查，但前者还要先解除占用，提示必须更具体。
  if (summary.files.some((file) => file.state === "unreadable")) {
    return { status: "blocked", reason: "unreadable" };
  }
  if (state?.stale) {
    return { status: "blocked", reason: "stale" };
  }
  const counts = externalAdoptCounts(summary);
  if (counts.claimable === 0) {
    return { status: "blocked", reason: "nothing_to_adopt" };
  }
  return { status: "available", counts };
}
