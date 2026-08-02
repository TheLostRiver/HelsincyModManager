import type {
  BatchModLifecycleAttemptStatus,
  BatchModLifecycleItemStatus,
  BatchModLifecycleOperation,
} from "./batchModLifecycleTypes";

export function getBatchOperationLabel(operation: BatchModLifecycleOperation): string {
  switch (operation) {
    case "install":
      return "批量安装";
    case "uninstall":
      return "批量卸载";
    case "reinstall":
      return "批量重装";
    default:
      return "批量操作";
  }
}

export function getBatchItemStatusLabel(status: BatchModLifecycleItemStatus): string {
  switch (status) {
    case "running":
      return "执行中";
    case "succeeded":
      return "成功";
    case "blocked":
      return "已阻止";
    case "failed":
      return "失败";
    case "recovery_required":
      return "需要恢复";
    case "cancelled":
      return "已取消";
    case "skipped":
      return "已跳过";
    default:
      return status;
  }
}

export function getBatchAttemptStatusLabel(status: BatchModLifecycleAttemptStatus): string {
  switch (status) {
    case "sealed":
      return "已封存";
    case "queued":
      return "排队中";
    case "running":
      return "执行中";
    case "stopping":
      return "停止中";
    case "completed":
      return "全部成功";
    case "completed_with_errors":
      return "部分成功";
    case "blocked":
      return "已被阻止";
    case "cancelled":
      return "已取消";
    case "recovery_required":
      return "需要恢复";
    case "interrupted":
      return "已中断";
    case "failed":
      return "失败";
    default:
      return status;
  }
}

export function getBatchExcludedReasonLabel(reason: string): string {
  switch (reason) {
    case "already_installed":
      return "已安装，不参与本次安装";
    case "not_installed":
      return "未安装，不参与本次卸载/重装";
    case "installed_revision_unavailable":
      return "已安装但缺少版本信息（旧格式清单），无法参与";
    default:
      return "不参与本次操作";
  }
}

export function getBatchErrorLabel(code: string): string {
  switch (code) {
    case "batch_no_applicable_items":
      return "选中的 Mod 均不适用于该操作";
    case "batch_facts_unavailable":
      return "无法读取安装状态或版本信息";
    case "batch_input_invalid":
      return "批量请求不合法";
    case "batch_duplicate_item":
      return "批量请求包含重复的 Mod";
    case "batch_resource_limit_exceeded":
      return "批量请求超出资源上限（最多 100 项）";
    case "batch_global_target_conflict":
      return "多个 Mod 的目标文件互相冲突";
    case "batch_plan_blocked":
      return "批量计划被阻止执行";
    case "batch_plan_stale":
      return "批量计划已过期，请重新预览";
    case "batch_plan_expired":
      return "批量计划已过期，请重新预览";
    case "batch_token_invalid":
      return "批量操作凭证无效";
    case "batch_retry_unavailable":
      return "当前没有可重试的项";
    case "batch_attempt_stale":
      return "已有更新的执行尝试，请刷新结果";
    case "batch_result_unavailable":
      return "无法读取批量执行结果";
    case "batch_journal_unavailable":
      return "批量执行记录不可用";
    case "batch_evidence_unavailable":
      return "批量执行证据不可用";
    case "sandbox_batch_production_forbidden":
      return "批量操作仅在测试环境可用";
    case "batch_internal_error":
      return "批量操作失败，请稍后重试";
    default:
      return "批量操作失败";
  }
}
