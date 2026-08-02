import { invoke } from "@tauri-apps/api/core";
import type { TaskStartedDto } from "../modImportTypes";
import {
  BATCH_MOD_LIFECYCLE_RESULT_PAGE_SIZE,
  type BatchModLifecyclePreviewDto,
  type BatchModLifecycleRequestDto,
  type BatchModLifecycleResultPageDto,
  type BatchModLifecycleSealDto,
  type BatchModLifecycleStartedDto,
} from "./batchModLifecycleTypes.ts";

export function previewBatchModLifecycle(
  request: BatchModLifecycleRequestDto,
): Promise<BatchModLifecyclePreviewDto> {
  return invoke<BatchModLifecyclePreviewDto>("preview_batch_mod_lifecycle", {
    request,
  });
}

export function sealBatchModLifecycle(input: {
  request: BatchModLifecycleRequestDto;
  previewToken: string;
}): Promise<BatchModLifecycleSealDto> {
  return invoke<BatchModLifecycleSealDto>("seal_batch_mod_lifecycle", {
    request: input.request,
    previewToken: input.previewToken,
  });
}

export function startBatchModLifecycle(input: {
  batchId: string;
  planToken: string;
}): Promise<BatchModLifecycleStartedDto> {
  return invoke<BatchModLifecycleStartedDto>("start_batch_mod_lifecycle", {
    batchId: input.batchId,
    planToken: input.planToken,
  });
}

export function getBatchModLifecycleResult(input: {
  batchId: string;
  attemptNumber: number;
  cursor?: string | null;
  limit?: number | null;
}): Promise<BatchModLifecycleResultPageDto> {
  return invoke<BatchModLifecycleResultPageDto>(
    "get_batch_mod_lifecycle_result",
    {
      batchId: input.batchId,
      attemptNumber: input.attemptNumber,
      cursor: input.cursor ?? null,
      limit: input.limit ?? BATCH_MOD_LIFECYCLE_RESULT_PAGE_SIZE,
    },
  );
}

export function retryBatchModLifecycle(input: {
  batchId: string;
  expectedAttemptNumber: number;
}): Promise<BatchModLifecycleStartedDto> {
  return invoke<BatchModLifecycleStartedDto>("retry_batch_mod_lifecycle", {
    batchId: input.batchId,
    expectedAttemptNumber: input.expectedAttemptNumber,
  });
}

/** Cancellation reuses the controlled `cancel_task` command. T13-06 batch `start`/`retry`
 *  execute synchronously and return terminal tasks, so `cancel_task` is not supported for
 *  batch tasks and returns `task_cannot_be_cancelled`; running cancellation arrives with the
 *  T13-07 asynchronous execution model. */
export function cancelBatchModLifecycleTask(input: {
  taskId: string;
}): Promise<TaskStartedDto> {
  return invoke<TaskStartedDto>("cancel_task", { taskId: input.taskId });
}
