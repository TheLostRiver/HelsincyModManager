import { autoUpdate, flip, FloatingPortal, offset, shift, size, useClick, useDismiss, useFloating, useInteractions, useRole } from "@floating-ui/react";
import { X } from "lucide-react";
import { useCallback, useId, useRef, useState, type ReactNode } from "react";
import { useModalFocusTrap } from "../../shared/feedback/useModalFocusTrap";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { retargetDialogCopy } from "./retargetDialogCopy";
import "./RetargetPopover.css";

/** 只管理展示与焦点；查询和保存控制器留在工作流中，开关浮层不重新加载。 */
export function RetargetPopover({ trigger, title, children, feedback }: { trigger: ReactNode; title: string; children: ReactNode; feedback?: ReactNode }) {
  const { locale } = useI18n();
  const copy = resolveCopy(retargetDialogCopy, locale);
  const [open, setOpen] = useState(false);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const titleId = useId();
  const close = useCallback(() => setOpen(false), []);
  const floating = useFloating({ open, onOpenChange: setOpen, placement: "bottom-end", strategy: "fixed", whileElementsMounted: autoUpdate,
    middleware: [offset(8), flip(), shift({ padding: 12 }), size({ padding: 12, apply: ({ availableHeight, elements }) => {
      elements.floating.style.setProperty("--retarget-popover-height", `${Math.max(0, availableHeight)}px`);
    } })] });
  const click = useClick(floating.context);
  const dismiss = useDismiss(floating.context, { escapeKey: false });
  const role = useRole(floating.context, { role: "dialog" });
  const { getReferenceProps, getFloatingProps } = useInteractions([click, dismiss, role]);
  useModalFocusTrap({ active: open, containerRef: panelRef, closeOnEscape: true, onRequestClose: close });

  return <>
    <button type="button" className="retarget-popover__trigger" ref={floating.refs.setReference} {...getReferenceProps()}>{trigger}</button>
    {!open && feedback}
    {open && <FloatingPortal preserveTabOrder={false}>
      <div {...getFloatingProps()} ref={(element) => { panelRef.current = element; floating.refs.setFloating(element); }} style={floating.floatingStyles}
        className="retarget-popover" aria-modal="true" aria-labelledby={titleId} tabIndex={-1}>
        <header><h4 id={titleId}>{title}</h4><button type="button" aria-label={copy.closeDetails} onClick={close}><X size={16} aria-hidden="true" /></button></header>
        <div className="retarget-popover__body">{feedback}{children}</div>
      </div>
    </FloatingPortal>}
  </>;
}
