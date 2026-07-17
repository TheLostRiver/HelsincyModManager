import type { ReactNode } from "react";

type TaskNoticeTone = "neutral" | "progress" | "success" | "warning" | "danger";

export type TaskNoticeProps = {
  taskId: string;
  title: string;
  message?: string;
  tone?: TaskNoticeTone;
  children?: ReactNode;
  actions?: ReactNode;
};

export function TaskNotice({
  taskId,
  title,
  message,
  tone = "neutral",
  children,
  actions,
}: TaskNoticeProps) {
  const role = tone === "danger" ? "alert" : "status";

  return (
    <section
      className={`feedback-task-notice is-${tone}`}
      data-task-id={taskId}
      role={role}
      aria-live={tone === "danger" ? "assertive" : "polite"}
      aria-atomic="true"
    >
      <div className="feedback-task-notice__copy">
        <strong>{title}</strong>
        {message ? <p>{message}</p> : null}
      </div>
      {children}
      {actions ? <div className="feedback-task-notice__actions">{actions}</div> : null}
    </section>
  );
}
