import { X } from "lucide-react";
import { useId, useRef, type ReactNode, type RefObject } from "react";
import { FeedbackPortal } from "./FeedbackProvider";
import { useModalFocusTrap } from "./useModalFocusTrap";

export type ModalSurfaceProps = {
  kind: "dialog" | "sheet";
  open: boolean;
  title: string;
  description?: string;
  icon?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
  onClose: () => void;
  closeLabel?: string;
  closeOnEscape?: boolean;
  closeOnBackdrop?: boolean;
  busy?: boolean;
  initialFocusRef?: RefObject<HTMLElement | null>;
  role?: "dialog" | "alertdialog";
};

export function ModalSurface({
  kind,
  open,
  title,
  description,
  icon,
  children,
  footer,
  onClose,
  closeLabel = "关闭",
  closeOnEscape = true,
  closeOnBackdrop = true,
  busy = false,
  initialFocusRef,
  role = "dialog",
}: ModalSurfaceProps) {
  const panelRef = useRef<HTMLElement | null>(null);
  const titleId = useId();
  const descriptionId = useId();
  const canClose = !busy;

  useModalFocusTrap({
    active: open,
    containerRef: panelRef,
    closeOnEscape: closeOnEscape && canClose,
    onRequestClose: onClose,
    initialFocusRef,
  });

  if (!open) {
    return null;
  }

  return (
    <FeedbackPortal>
      <div
        className={`feedback-overlay is-${kind}`}
        role="presentation"
        onPointerDown={(event) => {
          if (event.target === event.currentTarget && closeOnBackdrop && canClose) {
            onClose();
          }
        }}
      >
        <section
          ref={panelRef}
          className={`feedback-modal is-${kind}`}
          role={role}
          aria-modal="true"
          aria-labelledby={titleId}
          aria-describedby={description ? descriptionId : undefined}
          aria-busy={busy || undefined}
          tabIndex={-1}
        >
          <header className="feedback-modal__header">
            <div className="feedback-modal__heading">
              {icon ? <span className="feedback-modal__icon" aria-hidden="true">{icon}</span> : null}
              <div>
                <h2 id={titleId}>{title}</h2>
                {description ? <p id={descriptionId}>{description}</p> : null}
              </div>
            </div>
            <button
              type="button"
              className="feedback-modal__close"
              aria-label={closeLabel}
              title={closeLabel}
              disabled={!canClose}
              onClick={onClose}
            >
              <X size={18} />
            </button>
          </header>

          {children ? <div className="feedback-modal__body">{children}</div> : null}
          {footer ? <footer className="feedback-modal__footer">{footer}</footer> : null}
        </section>
      </div>
    </FeedbackPortal>
  );
}
