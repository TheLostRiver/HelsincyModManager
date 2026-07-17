export const FEEDBACK_TOAST_QUEUE_LIMIT = 4;
export const DEFAULT_FEEDBACK_TOAST_DURATION_MS = 6000;

export type FeedbackToastTone = "neutral" | "success" | "warning" | "danger";
export type FeedbackToastAction = { label: string; onSelect: () => void };
export type FeedbackToastInput = {
  eventKey: string;
  title: string;
  message: string;
  tone?: FeedbackToastTone;
  taskId?: string;
  durationMs?: number;
  action?: FeedbackToastAction;
};
export type FeedbackToastItem = FeedbackToastInput & {
  id: string;
  tone: FeedbackToastTone;
  durationMs: number;
  occurrences: number;
  revision: number;
};

export function enqueueFeedbackToast(queue: FeedbackToastItem[], input: FeedbackToastInput, sequence: number) {
  const existingIndex = queue.findIndex((toast) => toast.eventKey === input.eventKey);
  if (existingIndex >= 0) {
    const existing = queue[existingIndex];
    const merged: FeedbackToastItem = {
      ...input,
      id: existing.id,
      tone: input.tone ?? "neutral",
      durationMs: input.durationMs ?? DEFAULT_FEEDBACK_TOAST_DURATION_MS,
      occurrences: existing.occurrences + 1,
      revision: existing.revision + 1,
    };
    return [...queue.slice(0, existingIndex), ...queue.slice(existingIndex + 1), merged];
  }
  const next: FeedbackToastItem = {
    ...input,
    id: `feedback-toast-${sequence}`,
    tone: input.tone ?? "neutral",
    durationMs: input.durationMs ?? DEFAULT_FEEDBACK_TOAST_DURATION_MS,
    occurrences: 1,
    revision: 0,
  };
  return [...queue, next].slice(-FEEDBACK_TOAST_QUEUE_LIMIT);
}

export function dismissFeedbackToast(queue: FeedbackToastItem[], id: string) {
  return queue.filter((toast) => toast.id !== id);
}
