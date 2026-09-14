import { ListChecks, PanelRightClose } from "lucide-react";
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { retargetDialogCopy } from "./retargetDialogCopy";
import "./RetargetWorkspace.css";

type RetargetWorkspaceProps = {
  selection: ReactNode;
  preview: ReactNode;
  feedback: ReactNode;
  actions: ReactNode;
  previewStatus: "idle" | "loading" | "ready" | "error";
  labels?: { selection: string; preview: string; collapsePreview: string; showPreview: string };
};

/** 单来源和多来源共用布局；安装判断与任务状态仍由各自工作流提供。 */
export function RetargetWorkspace({ selection, preview, feedback, actions, previewStatus, labels }: RetargetWorkspaceProps) {
  const { locale } = useI18n();
  const copy = labels ?? resolveCopy(retargetDialogCopy, locale);
  const [previewOpen, setPreviewOpen] = useState(false);
  const narrow = useNarrowWorkspace();
  const previewRef = useRef<HTMLElement | null>(null);
  const selectionRef = useRef<HTMLElement | null>(null);
  const restoreSelectionFocus = useRef(false);
  const resetPreviewScroll = useRef(true);
  const selectionId = useId();
  const previewId = useId();

  useEffect(() => {
    resetPreviewScroll.current = true;
    setPreviewOpen(previewStatus !== "idle");
  }, [previewStatus]);

  useEffect(() => {
    if (previewOpen && (previewStatus === "ready" || previewStatus === "error")) {
      const pane = previewRef.current;
      if (pane) {
        if (resetPreviewScroll.current) pane.scrollTop = 0;
        resetPreviewScroll.current = false;
        pane.focus({ preventScroll: true });
      }
    }
  }, [previewOpen, previewStatus, narrow]);

  const hasPreview = previewStatus !== "idle";
  const showPreview = hasPreview && previewOpen;
  const selectionHidden = narrow && showPreview;
  useEffect(() => {
    if (showPreview || !restoreSelectionFocus.current) return;
    restoreSelectionFocus.current = false;
    const search = Array.from(selectionRef.current?.querySelectorAll<HTMLInputElement>('input[type="search"]') ?? [])
      .find((input) => !input.disabled && input.getClientRects().length > 0);
    (search ?? selectionRef.current)?.focus({ preventScroll: true });
  }, [showPreview]);
  const collapsePreview = () => {
    restoreSelectionFocus.current = true;
    setPreviewOpen(false);
  };

  return <div className="replacement-panel retarget-workspace" data-preview-open={showPreview}>
    <section id={selectionId} ref={selectionRef} className="retarget-workspace__selection" data-active={!showPreview}
      inert={selectionHidden} aria-hidden={selectionHidden || undefined} aria-label={copy.selection} tabIndex={-1}>
      {selection}
    </section>
    <div className="retarget-workspace__preview-frame">
      <section id={previewId} ref={previewRef} className="retarget-workspace__preview" data-active={showPreview}
        inert={!showPreview} aria-hidden={!showPreview} aria-label={copy.preview} tabIndex={-1}>
        <div className="retarget-workspace__preview-heading">
          <span><ListChecks size={16} aria-hidden="true" />{copy.preview}</span>
          <button type="button" aria-controls={selectionId} onClick={collapsePreview}><PanelRightClose size={16} aria-hidden="true" />{copy.collapsePreview}</button>
        </div>
        {preview}
      </section>
    </div>
    {feedback || actions ? <footer className="retarget-workspace__footer">
      <div className="retarget-workspace__feedback">{feedback}</div>
      <div className="retarget-workspace__controls">
        {hasPreview && !showPreview && <button type="button" className="retarget-workspace__show-preview" aria-expanded={showPreview} aria-controls={previewId}
          onClick={() => setPreviewOpen(true)}><ListChecks size={16} aria-hidden="true" />{copy.showPreview}</button>}
        <div className="replacement-panel__actions">{actions}</div>
      </div>
    </footer> : null}
  </div>;
}

/** 与 CSS 的窄窗断点同步：离场区立即退出键盘导航，视觉过渡由 CSS 自行完成。 */
function useNarrowWorkspace() {
  const query = "(max-width: 900px)";
  const [narrow, setNarrow] = useState(() =>
    typeof window !== "undefined" && typeof window.matchMedia === "function" && window.matchMedia(query).matches,
  );
  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") return;
    const media = window.matchMedia(query);
    const update = () => setNarrow(media.matches);
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  return narrow;
}
