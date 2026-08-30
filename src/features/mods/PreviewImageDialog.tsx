import { useCallback, useEffect, useRef, useState } from "react";
import { Minus, Plus, RotateCcw } from "lucide-react";
import { Dialog } from "../../shared/feedback/Dialog";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { getModDetailPreviewImage } from "./modPreviewImageApi";
import type { PreviewImage } from "./modPreviewImageTypes";
import { previewImageViewCopy } from "./previewImageViewCopy";
import {
  INITIAL_VIEW,
  MAX_SCALE,
  MIN_SCALE,
  SCALE_STEP,
  advanceDrag,
  isDefaultView,
  normalizeView,
  type DragAnchor,
  type Size,
  type ViewState,
} from "./previewImageZoom";
import "./PreviewImageDialog.css";

export type PreviewImageDialogProps = {
  modId: string;
  modName: string;
  fallbackThumbnailUrl: string;
  onClose: () => void;
};

export function PreviewImageDialog({
  modId,
  modName,
  fallbackThumbnailUrl,
  onClose,
}: PreviewImageDialogProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(previewImageViewCopy, locale);
  const [detail, setDetail] = useState<PreviewImage | null>(null);
  const [status, setStatus] = useState<"loading" | "ready" | "failed">("loading");
  const [view, setView] = useState<ViewState>(INITIAL_VIEW);
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const dragRef = useRef<DragAnchor | null>(null);
  const viewRef = useRef(view);
  viewRef.current = view;
  // Natural size of the loaded image. A ref, not state: it is only read when
  // clamping, so keeping it out of state avoids re-rendering on every load.
  const imageSizeRef = useRef<Size>({ width: 0, height: 0 });

  /**
   * Content box of the viewport: what the image's `max-width/max-height: 100%`
   * resolves against. `getBoundingClientRect()` would include the border.
   */
  const readViewport = useCallback((): Size => {
    const element = viewportRef.current;
    if (!element) return { width: 0, height: 0 };
    return { width: element.clientWidth, height: element.clientHeight };
  }, []);

  /** Applies `next` and returns the view that was actually committed. */
  const applyView = useCallback((next: ViewState): ViewState => {
    const applied = normalizeView(next, readViewport(), imageSizeRef.current);
    const current = viewRef.current;
    if (
      applied.scale === current.scale
      && applied.x === current.x
      && applied.y === current.y
    ) {
      return current;
    }
    setView(applied);
    return applied;
  }, [readViewport]);

  useEffect(() => {
    let cancelled = false;
    setStatus("loading");
    setDetail(null);
    setView(INITIAL_VIEW);
    imageSizeRef.current = { width: 0, height: 0 };

    getModDetailPreviewImage(modId)
      .then((image) => {
        if (cancelled) return;
        setDetail(image);
        setStatus("ready");
      })
      .catch(() => {
        if (cancelled) return;
        setStatus("failed");
      });

    return () => {
      cancelled = true;
    };
  }, [modId]);

  // React's onWheel is passive, so preventDefault needs a native listener.
  useEffect(() => {
    const element = viewportRef.current;
    if (!element) return undefined;
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      const current = viewRef.current;
      applyView({
        scale: current.scale * (event.deltaY < 0 ? SCALE_STEP : 1 / SCALE_STEP),
        x: current.x,
        y: current.y,
      });
    };
    element.addEventListener("wheel", onWheel, { passive: false });
    return () => element.removeEventListener("wheel", onWheel);
  }, [applyView]);

  // The pan range is derived from the viewport size, so resizing the window can
  // invalidate an offset that was legal a moment ago. Re-clamp instead of
  // letting the image sit half outside the frame.
  useEffect(() => {
    const element = viewportRef.current;
    if (!element) return undefined;
    const observer = new ResizeObserver(() => {
      applyView(viewRef.current);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [applyView]);

  const detailUrl = detail?.kind === "thumbnail" ? detail.thumbnailUrl : null;
  const imageSrc = detailUrl ?? fallbackThumbnailUrl;
  const hasImage = imageSrc.length > 0;
  const percent = Math.round(view.scale * 100);
  const viewIsDefault = isDefaultView(view);

  const hint = (() => {
    if (status === "loading") return copy.dialog.loading;
    if (status === "failed") return copy.dialog.loadFailed;
    if (detailUrl === null && hasImage) return copy.dialog.fallbackNotice;
    if (!hasImage) return copy.dialog.unavailable;
    return copy.dialog.dragHint;
  })();

  return (
    <Dialog
      open
      title={copy.dialog.title}
      description={modName}
      onClose={onClose}
      closeLabel={copy.dialog.closeAria}
      footer={
        <>
          <button
            type="button"
            className="preview-image-dialog__icon-button"
            onClick={() => applyView({ ...view, scale: view.scale / SCALE_STEP })}
            disabled={view.scale <= MIN_SCALE}
            aria-label={copy.dialog.zoomOut}
          >
            <Minus size={16} aria-hidden="true" />
          </button>
          <span className="preview-image-dialog__level" aria-live="polite">
            {copy.dialog.zoomLevel(percent)}
          </span>
          <button
            type="button"
            className="preview-image-dialog__icon-button"
            onClick={() => applyView({ ...view, scale: view.scale * SCALE_STEP })}
            disabled={view.scale >= MAX_SCALE}
            aria-label={copy.dialog.zoomIn}
          >
            <Plus size={16} aria-hidden="true" />
          </button>
          <button
            type="button"
            className="preview-image-dialog__reset"
            onClick={() => setView(INITIAL_VIEW)}
            disabled={viewIsDefault}
          >
            <RotateCcw size={14} aria-hidden="true" />
            {copy.dialog.reset}
          </button>
        </>
      }
    >
      <div
        ref={viewportRef}
        className="preview-image-dialog__viewport"
        onPointerDown={(event) => {
          dragRef.current = {
            px: event.clientX,
            py: event.clientY,
            ox: view.x,
            oy: view.y,
          };
          event.currentTarget.setPointerCapture(event.pointerId);
        }}
        onPointerMove={(event) => {
          const drag = dragRef.current;
          if (!drag) return;
          const stepped = advanceDrag(
            drag,
            { x: event.clientX, y: event.clientY },
            view.scale,
            readViewport(),
            imageSizeRef.current,
          );
          dragRef.current = stepped.anchor;
          setView(stepped.view);
        }}
        onPointerUp={() => {
          dragRef.current = null;
        }}
        onPointerCancel={() => {
          dragRef.current = null;
        }}
        onDoubleClick={() => setView(INITIAL_VIEW)}
      >
        {status === "loading" ? (
          <p className="preview-image-dialog__status">{copy.dialog.loading}</p>
        ) : null}
        {status !== "loading" && hasImage ? (
          <img
            className="preview-image-dialog__image"
            src={imageSrc}
            alt=""
            draggable={false}
            onLoad={(event) => {
              imageSizeRef.current = {
                width: event.currentTarget.naturalWidth,
                height: event.currentTarget.naturalHeight,
              };
              // The pan range is unknown until the size is known, so re-clamp
              // whatever the view is now.
              applyView(viewRef.current);
            }}
            style={{
              transform: `translate(${view.x}px, ${view.y}px) scale(${view.scale})`,
            }}
          />
        ) : null}
        {status === "ready" && !hasImage ? (
          <p className="preview-image-dialog__status">{copy.dialog.unavailable}</p>
        ) : null}
      </div>
      <p className="preview-image-dialog__hint">{hint}</p>
    </Dialog>
  );
}
