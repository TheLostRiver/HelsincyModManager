import type { ReactNode } from "react";
import { resolveCopy, useI18n } from "../i18n";
import { feedbackCopy } from "./feedbackCopy";
import { FeedbackPortal } from "./FeedbackProvider";

export type TaskNoticeViewportProps = {
  children?: ReactNode;
  label?: string;
};

export function TaskNoticeViewport({ children, label }: TaskNoticeViewportProps) {
  const { locale } = useI18n();
  const resolvedLabel = label ?? resolveCopy(feedbackCopy, locale).taskNoticeViewportLabel;
  return (
    <FeedbackPortal>
      <section className="feedback-task-notice-viewport" role="region" aria-label={resolvedLabel}>
        {children}
      </section>
    </FeedbackPortal>
  );
}
