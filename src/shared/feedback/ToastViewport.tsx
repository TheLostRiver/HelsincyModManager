import type { ReactNode } from "react";
import { FeedbackPortal } from "./FeedbackProvider";

export type ToastViewportProps = {
  children?: ReactNode;
  label?: string;
};

export function ToastViewport({ children, label = "通知" }: ToastViewportProps) {
  return (
    <FeedbackPortal>
      <section
        className="feedback-toast-viewport"
        role="region"
        aria-label={label}
        aria-live="polite"
        aria-atomic="false"
        aria-relevant="additions removals"
      >
        {children}
      </section>
    </FeedbackPortal>
  );
}
