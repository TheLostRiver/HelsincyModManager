import { AlertTriangle, CheckCircle2, Info, X } from "lucide-react";
import { useEffect, useState } from "react";
import type { FeedbackToastItem } from "./feedbackToastState";

export function FeedbackToast({ toast, onDismiss }: { toast: FeedbackToastItem; onDismiss: (id: string) => void }) {
  const [paused, setPaused] = useState(false);
  useEffect(() => {
    if (paused || toast.durationMs <= 0) return undefined;
    const timer = window.setTimeout(() => onDismiss(toast.id), toast.durationMs);
    return () => window.clearTimeout(timer);
  }, [onDismiss, paused, toast.durationMs, toast.id, toast.revision]);

  const icon = toast.tone === "success" ? <CheckCircle2 size={18} />
    : toast.tone === "warning" || toast.tone === "danger" ? <AlertTriangle size={18} /> : <Info size={18} />;
  return (
    <article
      className={`feedback-toast is-${toast.tone}`}
      data-toast-id={toast.id}
      data-event-key={toast.eventKey}
      data-task-id={toast.taskId}
      role={toast.tone === "danger" ? "alert" : "status"}
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocusCapture={() => setPaused(true)}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setPaused(false);
      }}
    >
      <span className="feedback-toast__icon" aria-hidden="true">{icon}</span>
      <div className="feedback-toast__copy">
        <strong>{toast.title}</strong>
        <p>{toast.message}</p>
        {toast.occurrences > 1 ? <small>已合并 {toast.occurrences} 次相同通知</small> : null}
      </div>
      <div className="feedback-toast__actions">
        {toast.action ? <button type="button" onClick={() => { toast.action?.onSelect(); onDismiss(toast.id); }}>{toast.action.label}</button> : null}
        <button type="button" onClick={() => onDismiss(toast.id)} aria-label="关闭通知" title="关闭通知">
          <X size={16} aria-hidden="true" />
        </button>
      </div>
    </article>
  );
}
