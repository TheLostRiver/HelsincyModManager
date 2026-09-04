import type { GameId, GameSetupErrorCode, GameSetupStatus } from "./gameSetupTypes";
import type { MappedCommandError } from "./gameSetupViewModel";

/**
 * 「手动保存游戏目录」这条操作链的状态迁移。
 *
 * 抽成纯函数是为了能被 `node --test` 直接驱动完整序列。#333 两次栽在同一处，
 * 第一次是逻辑错，第二次是测试根本测不到逻辑：原测试用 readFileSync + 正则匹配
 * hook 源码文本，只能证明「源码里存在某段三元表达式」，无法实例化 hook、无法观察
 * 运行时状态，于是缺陷带着绿灯合并。行为断言必须能跑到状态上，这个模块就是那个
 * 可跑到的落点。
 */
export type GameSetupSaveSlice = {
  status: GameSetupStatus;
  /**
   * `validating` 覆盖掉的上一个 status，只在保存进行中非 null。
   *
   * 为什么需要它：`validating` 是前端合成的瞬时态（后端 `GameSetupStatusDto.kind`
   * 只有 not_configured / invalid / configured），保存一开始就把它写进 `status`。
   * #333 第一版修复在失败分支判 `status.kind === "configured"` 来决定是否保留配置，
   * 可那时 `status` 早已是 `validating`，判断恒假——乐观转场把判据自己擦掉了。
   * 判据必须存在一个 `validating` 不会碰的地方。
   */
  statusBeforeSave: GameSetupStatus | null;
  lastSaveError: GameSetupErrorCode | null;
  isBusy: boolean;
};

/**
 * 进入校验中。原 status 存入 statusBeforeSave 以备失败还原。
 *
 * 已在 validating 时保留既有 statusBeforeSave，不让 validating 二次污染它：
 * UI 上 GameDirectoryActions 三个按钮都 `disabled={isBusy}`，重入本已挡住，但状态机
 * 自身不该依赖 UI 兜底——这正是第一版的教训。
 */
export function beginDirectorySave<T extends GameSetupSaveSlice>(slice: T, gameId: GameId): T {
  return {
    ...slice,
    status: { kind: "validating", gameId },
    statusBeforeSave: slice.status.kind === "validating" ? slice.statusBeforeSave : slice.status,
    isBusy: true,
  };
}

/** 保存成功：后端返回的 status 即权威现状，上次失败已不再是现状。 */
export function completeDirectorySave<T extends GameSetupSaveSlice>(
  slice: T,
  status: GameSetupStatus,
): T {
  return {
    ...slice,
    status,
    statusBeforeSave: null,
    lastSaveError: null,
    isBusy: false,
  };
}

/**
 * 保存失败：不改写已配置的 status，失败原因只进 lastSaveError。
 *
 * 后端在落盘之前就拒绝了（game_setup.rs 的 save_game_directory 把 validate_directory
 * 与存储重叠校验都排在 repository.save_game_instance 之前），磁盘上的配置原封不动，
 * UI 也就没有任何理由假装它丢了。把 status 拍成 invalid 会连带禁掉一键启动、恢复中心
 * 与安装健康检测——它们都以 `status.kind === "configured"` 为闸门，且必须重启 HMM
 * 才能恢复（#333）。
 *
 * 未配置 / invalid 态则仍转 invalid：此时没有既有配置需要保护，报告校验失败才是对的。
 */
export function failDirectorySave<T extends GameSetupSaveSlice>(
  slice: T,
  gameId: GameId,
  error: MappedCommandError,
): T {
  /* 没有前置 begin 时退回当前 status，让本函数单独调用也不会读到 undefined。 */
  const restored = slice.statusBeforeSave ?? slice.status;

  return {
    ...slice,
    status:
      restored.kind === "configured"
        ? restored
        : {
            kind: "invalid",
            gameId,
            errorCode: error.code,
            backendMessage: error.backendMessage,
          },
    statusBeforeSave: null,
    lastSaveError: error.code,
    isBusy: false,
  };
}
