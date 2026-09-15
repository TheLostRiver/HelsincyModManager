import type { ReactNode } from "react";
import { X } from "lucide-react";

type TaskNoticeTone = "neutral" | "progress" | "success" | "warning" | "danger";

export type TaskNoticeProps = {
  taskId: string;
  title: string;
  message?: string;
  tone?: TaskNoticeTone;
  children?: ReactNode;
  actions?: ReactNode;
  dismiss?: { label: string; onClick: () => void };
};

export function TaskNotice({
  taskId,
  title,
  message,
  tone = "neutral",
  children,
  actions,
  dismiss,
}: TaskNoticeProps) {
  const role = tone === "danger" ? "alert" : "status";

  return (
    <section
      className={`feedback-task-notice is-${tone}`}
      data-task-id={taskId}
      role={role}
      aria-live={tone === "danger" ? "assertive" : "polite"}
      aria-atomic="true"
      data-dismissible={dismiss ? "true" : undefined}
    >
      <div className="feedback-task-notice__copy">
        <strong>{title}</strong>
        {message ? <p>{message}</p> : null}
      </div>
      {dismiss ? (
        <button
          type="button"
          className="feedback-task-notice__dismiss"
          aria-label={dismiss.label}
          title={dismiss.label}
          onClick={dismiss.onClick}
        >
          <X size={16} aria-hidden="true" />
        </button>
      ) : null}
      {children}
      {actions ? <div className="feedback-task-notice__actions">{actions}</div> : null}
    </section>
  );
}
