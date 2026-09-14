import { ArrowRightLeft, ListChecks, Target } from "lucide-react";
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
};

/** 单来源和多来源共用布局；安装判断与任务状态仍由各自工作流提供。 */
export function RetargetWorkspace({ selection, preview, feedback, actions, previewStatus }: RetargetWorkspaceProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(retargetDialogCopy, locale);
  const [activePane, setActivePane] = useState<"selection" | "preview">("selection");
  const previewRef = useRef<HTMLElement | null>(null);
  const selectionId = useId();
  const previewId = useId();

  useEffect(() => {
    if (previewStatus !== "idle") setActivePane("preview");
  }, [previewStatus]);

  useEffect(() => {
    if (activePane === "preview" && (previewStatus === "ready" || previewStatus === "error")) {
      const pane = previewRef.current;
      if (pane) {
        pane.scrollTop = 0;
        pane.focus({ preventScroll: true });
      }
    }
  }, [activePane, previewStatus]);

  return <div className="replacement-panel retarget-workspace">
    <nav className="retarget-workspace__navigation" aria-label={copy.title}>
      <button type="button" aria-pressed={activePane === "selection"} aria-controls={selectionId} onClick={() => setActivePane("selection")}>
        <Target size={16} aria-hidden="true" />{copy.selection}
      </button>
      <button type="button" aria-pressed={activePane === "preview"} aria-controls={previewId} onClick={() => setActivePane("preview")}>
        <ListChecks size={16} aria-hidden="true" />{copy.preview}
      </button>
    </nav>
    <section id={selectionId} className="retarget-workspace__selection" data-active={activePane === "selection"} aria-label={copy.selection}>
      {selection}
    </section>
    <section id={previewId} ref={previewRef} className="retarget-workspace__preview" data-active={activePane === "preview"} aria-label={copy.preview} tabIndex={-1}>
      {previewStatus === "idle" ? <div className="retarget-workspace__empty">
        <span className="retarget-workspace__empty-icon"><ArrowRightLeft size={28} aria-hidden="true" /></span>
        <h3>{copy.emptyTitle}</h3>
        <p>{copy.emptyDescription}</p>
      </div> : preview}
    </section>
    {feedback || actions ? <footer className="retarget-workspace__footer">
      <div className="retarget-workspace__feedback">{feedback}</div>
      <div className="replacement-panel__actions">{actions}</div>
    </footer> : null}
  </div>;
}
