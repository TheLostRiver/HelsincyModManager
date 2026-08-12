import type { BackgroundProtectionControlDto } from "./backgroundProtectionTypes";

export type BackgroundProtectionPanelState =
  | { status: "loading" }
  | {
      status: "ready";
      control: BackgroundProtectionControlDto;
      actionErrorCode: string | null;
      refreshWarningCode: string | null;
    }
  | { status: "error"; errorCode: string };

export function readyBackgroundProtectionPanelState(
  control: BackgroundProtectionControlDto,
  actionErrorCode: string | null = null,
): BackgroundProtectionPanelState {
  return {
    status: "ready",
    control,
    actionErrorCode,
    refreshWarningCode: null,
  };
}

export function preserveBackgroundProtectionStateAfterRefreshFailure(
  current: BackgroundProtectionPanelState,
  errorCode: string,
): BackgroundProtectionPanelState {
  if (current.status !== "ready") {
    return { status: "error", errorCode };
  }

  return {
    ...current,
    refreshWarningCode: errorCode,
  };
}
