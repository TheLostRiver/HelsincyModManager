import { X } from "lucide-react";
import { resolveCopy, useI18n } from "../i18n";
import { feedbackCopy } from "./feedbackCopy";
import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
  type RefObject,
  type SyntheticEvent,
} from "react";
import { FeedbackPortal } from "./FeedbackProvider";
import { useModalFocusTrap } from "./useModalFocusTrap";

type ModalPhase = "closed" | "opening" | "open" | "settled" | "closing";

const MODAL_TRANSITION_MS = 200;
const REDUCED_MOTION_TRANSITION_MS = 140;

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
  /**
   * 追加到面板上的类名，用于覆盖默认尺寸。
   *
   * `dialog` 档是 560px 居中、`sheet` 档是右侧贴边，两者都不适合内容本身就需要大面积的
   * 场景（如包内容树）。这类面板自带一个类名来放大尺寸，而不是把新档位塞进 `kind`——
   * 尺寸是调用方的诉求，交互形态才是 `kind` 该管的事。
   */
  panelClassName?: string;
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
  closeLabel,
  closeOnEscape = true,
  closeOnBackdrop = true,
  busy = false,
  initialFocusRef,
  role = "dialog",
  panelClassName,
}: ModalSurfaceProps) {
  const { locale } = useI18n();
  const resolvedCloseLabel = closeLabel ?? resolveCopy(feedbackCopy, locale).modalCloseLabel;
  const panelRef = useRef<HTMLElement | null>(null);
  const phaseRef = useRef<ModalPhase>(open ? "opening" : "closed");
  const openingFrameRef = useRef<number | null>(null);
  const openFrameRef = useRef<number | null>(null);
  const settleTimerRef = useRef<number | null>(null);
  const closeTimerRef = useRef<number | null>(null);
  const [phase, setPhase] = useState<ModalPhase>(phaseRef.current);
  const titleId = useId();
  const descriptionId = useId();
  const canClose = !busy;

  const updatePhase = useCallback((nextPhase: ModalPhase) => {
    phaseRef.current = nextPhase;
    setPhase(nextPhase);
  }, []);

  const clearTransitionWork = useCallback(() => {
    if (openingFrameRef.current !== null) {
      window.cancelAnimationFrame(openingFrameRef.current);
      openingFrameRef.current = null;
    }
    if (openFrameRef.current !== null) {
      window.cancelAnimationFrame(openFrameRef.current);
      openFrameRef.current = null;
    }
    if (settleTimerRef.current !== null) {
      window.clearTimeout(settleTimerRef.current);
      settleTimerRef.current = null;
    }
    if (closeTimerRef.current !== null) {
      window.clearTimeout(closeTimerRef.current);
      closeTimerRef.current = null;
    }
  }, []);

  useEffect(() => {
    clearTransitionWork();

    if (!open) {
      if (phaseRef.current === "closed") {
        return undefined;
      }

      const transitionDurationMs = getModalTransitionMillis();
      updatePhase("closing");
      closeTimerRef.current = window.setTimeout(() => {
        closeTimerRef.current = null;
        updatePhase("closed");
      }, transitionDurationMs);
      return clearTransitionWork;
    }

    const transitionDurationMs = getModalTransitionMillis();
    updatePhase("opening");
    openingFrameRef.current = window.requestAnimationFrame(() => {
      openingFrameRef.current = null;
      openFrameRef.current = window.requestAnimationFrame(() => {
        openFrameRef.current = null;
        updatePhase("open");
        settleTimerRef.current = window.setTimeout(() => {
          settleTimerRef.current = null;
          if (phaseRef.current === "open") {
            updatePhase("settled");
          }
        }, transitionDurationMs);
      });
    });

    return clearTransitionWork;
  }, [clearTransitionWork, open, updatePhase]);

  useEffect(() => clearTransitionWork, [clearTransitionWork]);

  useModalFocusTrap({
    active: phase !== "closed",
    containerRef: panelRef,
    closeOnEscape: closeOnEscape && canClose && phase !== "closing",
    onRequestClose: onClose,
    initialFocusRef,
  });

  if (phase === "closed") {
    return null;
  }

  const transitionStyle = {
    "--feedback-modal-transition-duration": `${getModalTransitionMillis()}ms`,
  } as CSSProperties;
  const blockInteractionWhileClosing = (event: SyntheticEvent) => {
    if (phase !== "closing") {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
  };

  return (
    <FeedbackPortal>
      <div
        className={`feedback-overlay is-${kind} is-${phase}`}
        style={transitionStyle}
        role="presentation"
        onClickCapture={blockInteractionWhileClosing}
        onKeyDownCapture={blockInteractionWhileClosing}
        onPointerDown={(event) => {
          if (
            phase !== "closing"
            && event.target === event.currentTarget
            && closeOnBackdrop
            && canClose
          ) {
            onClose();
          }
        }}
      >
        <section
          ref={panelRef}
          className={`feedback-modal is-${kind}${panelClassName ? ` ${panelClassName}` : ""}`}
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
              aria-label={resolvedCloseLabel}
              title={resolvedCloseLabel}
              disabled={!canClose || phase === "closing"}
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

function getModalTransitionMillis() {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return MODAL_TRANSITION_MS;
  }

  return window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ? REDUCED_MOTION_TRANSITION_MS
    : MODAL_TRANSITION_MS;
}
