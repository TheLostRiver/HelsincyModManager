import { AlertTriangle, CheckCircle2, Info, X } from "lucide-react";
import { resolveCopy, useI18n } from "../i18n";
import { feedbackCopy } from "./feedbackCopy";
import { useCallback, useEffect, useRef, useState } from "react";
import type { FeedbackToastItem } from "./feedbackToastState";

/*
 * 退场动画时长。纯 CSS 无法为卸载中的节点播放动画（React 移除后节点已不存在），
 * 因此先标记退场、等动画播完再真正移除。该值必须与 feedback.css 中
 * .feedback-toast.is-exiting 的动画时长保持一致。
 */
const TOAST_EXIT_DURATION_MS = 160;

export function FeedbackToast({ toast, onDismiss }: { toast: FeedbackToastItem; onDismiss: (id: string) => void }) {
  const { locale } = useI18n();
  const copy = resolveCopy(feedbackCopy, locale);
  const [paused, setPaused] = useState(false);
  const [exiting, setExiting] = useState(false);
  const exitTimerRef = useRef<number | null>(null);

  const clearExitTimer = useCallback(() => {
    if (exitTimerRef.current !== null) {
      window.clearTimeout(exitTimerRef.current);
      exitTimerRef.current = null;
    }
  }, []);

  const requestDismiss = useCallback(() => {
    // 重入保护：退场进行中再次触发（连点关闭、或退场期间自动超时）不得重复计时。
    if (exitTimerRef.current !== null) {
      return;
    }
    setExiting(true);
    exitTimerRef.current = window.setTimeout(() => {
      exitTimerRef.current = null;
      onDismiss(toast.id);
    }, TOAST_EXIT_DURATION_MS);
  }, [onDismiss, toast.id]);

  /*
   * 同一条 toast 因重复事件被合并重放时（revision 变化），取消进行中的退场，
   * 否则它会在刚被复用后立刻消失。
   */
  useEffect(() => {
    setExiting(false);
    clearExitTimer();
  }, [clearExitTimer, toast.revision]);

  useEffect(() => clearExitTimer, [clearExitTimer]);

  useEffect(() => {
    if (paused || exiting || toast.durationMs <= 0) return undefined;
    const timer = window.setTimeout(requestDismiss, toast.durationMs);
    return () => window.clearTimeout(timer);
  }, [exiting, paused, requestDismiss, toast.durationMs, toast.revision]);

  const icon = toast.tone === "success" ? <CheckCircle2 size={18} />
    : toast.tone === "warning" || toast.tone === "danger" ? <AlertTriangle size={18} /> : <Info size={18} />;
  return (
    <article
      className={`feedback-toast is-${toast.tone}${exiting ? " is-exiting" : ""}`}
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
        {toast.occurrences > 1 ? <small>{copy.toastMerged(toast.occurrences)}</small> : null}
      </div>
      <div className="feedback-toast__actions">
        {toast.action ? <button type="button" onClick={() => { toast.action?.onSelect(); requestDismiss(); }}>{toast.action.label}</button> : null}
        <button type="button" onClick={requestDismiss} aria-label={copy.toastDismissAria} title={copy.toastDismissAria}>
          <X size={16} aria-hidden="true" />
        </button>
      </div>
    </article>
  );
}
