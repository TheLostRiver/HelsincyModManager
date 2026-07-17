import type { ReactNode } from "react";
import { FeedbackPortal } from "./FeedbackProvider";

export type TaskNoticeViewportProps = {
  children?: ReactNode;
  label?: string;
};

export function TaskNoticeViewport({ children, label = "任务进度" }: TaskNoticeViewportProps) {
  return (
    <FeedbackPortal>
      <section className="feedback-task-notice-viewport" role="region" aria-label={label}>
        {children}
      </section>
    </FeedbackPortal>
  );
}
