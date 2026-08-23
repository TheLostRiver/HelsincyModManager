import type { ReactNode } from "react";
import { resolveCopy, useI18n } from "../i18n";
import { feedbackCopy } from "./feedbackCopy";
import { FeedbackPortal } from "./FeedbackProvider";

export type ToastViewportProps = {
  children?: ReactNode;
  label?: string;
};

export function ToastViewport({ children, label }: ToastViewportProps) {
  const { locale } = useI18n();
  const resolvedLabel = label ?? resolveCopy(feedbackCopy, locale).toastViewportLabel;
  return (
    <FeedbackPortal>
      <section
        className="feedback-toast-viewport"
        role="region"
        aria-label={resolvedLabel}
        aria-live="polite"
        aria-atomic="false"
        aria-relevant="additions removals"
      >
        {children}
      </section>
    </FeedbackPortal>
  );
}
