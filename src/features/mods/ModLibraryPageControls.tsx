import { Check, ChevronDown } from "lucide-react";
import { useCallback, useEffect, useId, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { modLibraryCopy } from "./modLibraryCopy";
import { MOD_LIBRARY_PAGE_SIZES, getModLibraryItemRange, type ModLibraryPageSize } from "./modLibraryPaginationModel";
import type { ModLibraryPage } from "./modLibraryTypes";
import "./ModLibraryPageControls.css";

type ModLibraryPageControlsProps = {
  pageSize: ModLibraryPageSize;
  result: Pick<ModLibraryPage, "page" | "pageSize" | "matchingTotal"> | null;
  busy?: boolean;
  onPageSizeChange: (pageSize: ModLibraryPageSize) => void;
};

export function ModLibraryPageControls({
  pageSize, result, busy = false, onPageSizeChange,
}: ModLibraryPageControlsProps) {
  const { locale } = useI18n();
  const pagination = resolveCopy(modLibraryCopy, locale).pagination;
  const matchingTotal = result?.matchingTotal ?? 0;
  // 容量偏好可能已改变，范围仍以正在显示的查询快照为准。
  const range = getModLibraryItemRange(result?.page ?? 1, result?.pageSize ?? pageSize, matchingTotal);
  const completeRange = matchingTotal === 0
    ? pagination.emptyRange : pagination.range(range.start, range.end, matchingTotal);
  const rangeAnnouncement = result === null
    ? (busy ? pagination.busyLabel : "")
    : busy ? pagination.busyRange(completeRange) : completeRange;
  const pageSizeListboxId = useId();
  const pageSizeRootRef = useRef<HTMLDivElement | null>(null);
  const pageSizeTriggerRef = useRef<HTMLButtonElement | null>(null);
  const pageSizeOptionRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const [pageSizeMenuOpen, setPageSizeMenuOpen] = useState(false);
  const [focusedPageSizeIndex, setFocusedPageSizeIndex] = useState(() =>
    Math.max(0, MOD_LIBRARY_PAGE_SIZES.indexOf(pageSize)),
  );

  const closePageSizeMenu = useCallback((restoreTriggerFocus: boolean) => {
    setPageSizeMenuOpen(false);
    if (restoreTriggerFocus) {
      requestAnimationFrame(() => pageSizeTriggerRef.current?.focus());
    }
  }, []);

  const openPageSizeMenu = useCallback(() => {
    if (busy) {
      return;
    }
    setFocusedPageSizeIndex(Math.max(0, MOD_LIBRARY_PAGE_SIZES.indexOf(pageSize)));
    setPageSizeMenuOpen(true);
  }, [busy, pageSize]);

  useEffect(() => {
    if (busy && pageSizeMenuOpen) {
      closePageSizeMenu(true);
    }
  }, [busy, closePageSizeMenu, pageSizeMenuOpen]);

  useEffect(() => {
    if (!pageSizeMenuOpen) {
      return;
    }

    pageSizeOptionRefs.current[focusedPageSizeIndex]?.focus();
  }, [focusedPageSizeIndex, pageSizeMenuOpen]);

  useEffect(() => {
    if (!pageSizeMenuOpen) {
      return;
    }

    const handlePointerDown = (event: PointerEvent) => {
      if (!pageSizeRootRef.current?.contains(event.target as Node)) {
        closePageSizeMenu(false);
      }
    };
    const handleEscape = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        closePageSizeMenu(true);
      }
    };

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleEscape);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleEscape);
    };
  }, [closePageSizeMenu, pageSizeMenuOpen]);

  const focusPageSizeOption = (index: number) => {
    const optionCount = MOD_LIBRARY_PAGE_SIZES.length;
    const nextIndex = (index + optionCount) % optionCount;
    setFocusedPageSizeIndex(nextIndex);
    pageSizeOptionRefs.current[nextIndex]?.focus();
  };

  const commitPageSize = (nextPageSize: ModLibraryPageSize) => {
    if (busy) {
      closePageSizeMenu(false);
      return;
    }
    if (nextPageSize !== pageSize) {
      onPageSizeChange(nextPageSize);
    }
    closePageSizeMenu(true);
  };

  const handlePageSizeOptionKeyDown = (
    event: ReactKeyboardEvent<HTMLButtonElement>,
    optionIndex: number,
  ) => {
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        event.stopPropagation();
        focusPageSizeOption(optionIndex + 1);
        break;
      case "ArrowUp":
        event.preventDefault();
        event.stopPropagation();
        focusPageSizeOption(optionIndex - 1);
        break;
      case "Home":
        event.preventDefault();
        event.stopPropagation();
        focusPageSizeOption(0);
        break;
      case "End":
        event.preventDefault();
        event.stopPropagation();
        focusPageSizeOption(MOD_LIBRARY_PAGE_SIZES.length - 1);
        break;
      case "Enter":
      case " ":
        event.preventDefault();
        event.stopPropagation();
        commitPageSize(MOD_LIBRARY_PAGE_SIZES[optionIndex]);
        break;
      case "Escape":
        event.preventDefault();
        event.stopPropagation();
        closePageSizeMenu(true);
        break;
      default:
        break;
    }
  };

  return (
    <div className="mod-library-page-controls">
      <div
        className="mod-library-page-controls__page-size"
        ref={pageSizeRootRef}
        onBlur={(event) => {
          if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
            setPageSizeMenuOpen(false);
          }
        }}
      >
        <span className="mod-library-page-controls__segment-label">{pagination.perPage}</span>
        <span className="mod-library-page-controls__page-size-anchor">
          <button
            ref={pageSizeTriggerRef}
            type="button"
            className={`mod-library-page-controls__page-size-trigger${pageSizeMenuOpen && !busy ? " is-open" : ""}`}
            aria-label={pagination.perPageSizeAria(pageSize)}
            aria-haspopup="listbox"
            aria-expanded={pageSizeMenuOpen && !busy}
            aria-controls={pageSizeListboxId}
            aria-disabled={busy || undefined}
            onClick={() => {
              if (pageSizeMenuOpen) {
                closePageSizeMenu(false);
              } else {
                openPageSizeMenu();
              }
            }}
            onKeyDown={(event) => {
              if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                event.preventDefault();
                openPageSizeMenu();
              }
            }}
          >
            <span>{pagination.items(pageSize)}</span>
            <ChevronDown size={14} strokeWidth={2.25} aria-hidden="true" />
          </button>

          {pageSizeMenuOpen && !busy ? (
            <div
              className="mod-library-page-controls__page-size-listbox"
              id={pageSizeListboxId}
              role="listbox"
              aria-label={pagination.perPageCountAria}
            >
              {MOD_LIBRARY_PAGE_SIZES.map((option, optionIndex) => {
                const selected = option === pageSize;
                return (
                  <button
                    key={option}
                    ref={(node) => {
                      pageSizeOptionRefs.current[optionIndex] = node;
                    }}
                    type="button"
                    className={`mod-library-page-controls__page-size-option${selected ? " is-selected" : ""}`}
                    role="option"
                    aria-selected={selected}
                    tabIndex={focusedPageSizeIndex === optionIndex ? 0 : -1}
                    onFocus={() => setFocusedPageSizeIndex(optionIndex)}
                    onKeyDown={(event) => handlePageSizeOptionKeyDown(event, optionIndex)}
                    onClick={() => commitPageSize(option)}
                  >
                    <span>{pagination.items(option)}</span>
                    <Check size={14} strokeWidth={2.5} aria-hidden="true" />
                  </button>
                );
              })}
            </div>
          ) : null}
        </span>
      </div>

      <div className="mod-library-page-controls__range">
        {busy ? <span className="mod-library-page-controls__busy-indicator" aria-hidden="true" /> : null}
        <span className="mod-library-page-controls__range-compact" aria-hidden="true">
          {result === null ? pagination.compactEmpty
            : matchingTotal <= result.pageSize ? pagination.items(matchingTotal)
            : pagination.compactRange(range.start, range.end, matchingTotal)}
        </span>
        <span className="mod-library-page-controls__range-live" aria-live="polite" aria-atomic="true">
          {rangeAnnouncement}
        </span>
      </div>
    </div>
  );
}
