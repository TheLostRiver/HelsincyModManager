import {
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
  Check,
} from "lucide-react";
import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import {
  MOD_LIBRARY_PAGE_SIZES,
  getModLibraryEllipsisTarget,
  getModLibraryItemRange,
  getModLibraryPageSlots,
  getModLibraryTotalPages,
  type ModLibraryPageSize,
} from "./modLibraryPaginationModel";
import { ModLibraryControlTooltip } from "./ModLibraryControlTooltip";
import "./ModLibraryPagination.css";

export type ModLibraryPaginationProps = {
  page: number;
  pageSize: ModLibraryPageSize;
  matchingTotal: number;
  busy?: boolean;
  onPageChange: (page: number) => void;
  onPageSizeChange: (pageSize: ModLibraryPageSize) => void;
};

function clampPage(page: number, totalPages: number) {
  if (totalPages === 0) {
    return 0;
  }

  return Math.min(Math.max(1, Math.floor(page)), totalPages);
}

function getRangeAnnouncement(start: number, end: number, matchingTotal: number, busy: boolean) {
  const range =
    matchingTotal === 0
      ? "当前没有匹配的 Mod"
      : `显示第 ${start} 至 ${end} 项，共 ${matchingTotal} 项`;

  return busy ? `正在更新结果。${range}` : range;
}

export function ModLibraryPagination({
  page,
  pageSize,
  matchingTotal,
  busy = false,
  onPageChange,
  onPageSizeChange,
}: ModLibraryPaginationProps) {
  const totalPages = getModLibraryTotalPages(matchingTotal, pageSize);
  const currentPage = clampPage(page, totalPages);
  const pageSlots = getModLibraryPageSlots(currentPage, totalPages);
  const range = getModLibraryItemRange(currentPage, pageSize, matchingTotal);
  const rangeAnnouncement = getRangeAnnouncement(range.start, range.end, matchingTotal, busy);
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

  const requestPage = (nextPage: number) => {
    if (busy || nextPage < 1 || nextPage > totalPages || nextPage === currentPage) {
      return;
    }
    onPageChange(nextPage);
  };

  const previousDisabled = busy || currentPage <= 1;
  const nextDisabled = busy || currentPage === 0 || currentPage >= totalPages;

  return (
    <footer className="mod-library-pagination" aria-label="Mod 库分页工具栏">
      <div className="mod-library-pagination__layout">
        <div
          className="mod-library-pagination__page-size"
          ref={pageSizeRootRef}
          onBlur={(event) => {
            if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
              setPageSizeMenuOpen(false);
            }
          }}
        >
          <span className="mod-library-pagination__segment-label">每页</span>
          {/*
           * 触发器与浮层单独包一层定位锚点：浮层原先靠外层容器加 left: 22px 硬偏移
           * 才勉强对齐触发器，"每页"两字的宽度一变就会错位。
           */}
          <span className="mod-library-pagination__page-size-anchor">
          <button
            ref={pageSizeTriggerRef}
            type="button"
            className={`mod-library-pagination__page-size-trigger${pageSizeMenuOpen && !busy ? " is-open" : ""}`}
            aria-label={`每页显示 ${pageSize} 项`}
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
            <span>{pageSize} 项</span>
            <ChevronDown size={14} strokeWidth={2.25} aria-hidden="true" />
          </button>

          {pageSizeMenuOpen && !busy ? (
            <div
              className="mod-library-pagination__page-size-listbox"
              id={pageSizeListboxId}
              role="listbox"
              aria-label="每页显示数量"
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
                    className={`mod-library-pagination__page-size-option${selected ? " is-selected" : ""}`}
                    role="option"
                    aria-selected={selected}
                    tabIndex={focusedPageSizeIndex === optionIndex ? 0 : -1}
                    onFocus={() => setFocusedPageSizeIndex(optionIndex)}
                    onKeyDown={(event) => handlePageSizeOptionKeyDown(event, optionIndex)}
                    onClick={() => commitPageSize(option)}
                  >
                    <span>{option} 项</span>
                    <Check size={14} strokeWidth={2.5} aria-hidden="true" />
                  </button>
                );
              })}
            </div>
          ) : null}
          </span>
        </div>

        <nav
          className="mod-library-pagination__navigation"
          aria-label="Mod 库页码"
          aria-busy={busy}
        >
          <ModLibraryControlTooltip content="第一页" describeControl={false}>
            {() => (
              <button
                type="button"
                className="mod-library-pagination__icon-button"
                aria-label="前往第一页"
                aria-disabled={previousDisabled || undefined}
                onClick={() => requestPage(1)}
              >
                <ChevronsLeft size={16} strokeWidth={2.25} aria-hidden="true" />
              </button>
            )}
          </ModLibraryControlTooltip>
          <ModLibraryControlTooltip content="上一页" describeControl={false}>
            {() => (
              <button
                type="button"
                className="mod-library-pagination__icon-button"
                aria-label="前往上一页"
                aria-disabled={previousDisabled || undefined}
                onClick={() => requestPage(currentPage - 1)}
              >
                <ChevronLeft size={16} strokeWidth={2.25} aria-hidden="true" />
              </button>
            )}
          </ModLibraryControlTooltip>

          <div className="mod-library-pagination__page-list" aria-label="可选页码">
            {pageSlots.map((slot, index) =>
              slot === "ellipsis" ? (
                (() => {
                  /*
                   * 省略号原本是纯装饰文本，却占满一个按钮位——想跳到中间页只能连点上一页。
                   * 改为可点按钮，跳到被折叠区间的中点；无法推导目标时退回不可交互的文本，
                   * 避免出现点了却停在原地的按钮。
                   */
                  const target = getModLibraryEllipsisTarget(pageSlots, index);

                  if (target === null) {
                    return (
                      <span
                        key={`ellipsis-${index}`}
                        className="mod-library-pagination__ellipsis"
                        aria-hidden="true"
                      >
                        …
                      </span>
                    );
                  }

                  return (
                    <ModLibraryControlTooltip
                      key={`ellipsis-${index}`}
                      content={`跳至第 ${target} 页`}
                      describeControl={false}
                    >
                      {() => (
                        <button
                          type="button"
                          className="mod-library-pagination__ellipsis is-interactive"
                          aria-label={`跳至第 ${target} 页`}
                          aria-disabled={busy || undefined}
                          onClick={() => requestPage(target)}
                        >
                          …
                        </button>
                      )}
                    </ModLibraryControlTooltip>
                  );
                })()
              ) : (
                <button
                  key={slot}
                  type="button"
                  className="mod-library-pagination__page-button"
                  aria-label={`第 ${slot} 页`}
                  aria-current={slot === currentPage ? "page" : undefined}
                  aria-disabled={busy || undefined}
                  onClick={() => requestPage(slot)}
                >
                  {slot}
                </button>
              ),
            )}
          </div>

          <ModLibraryControlTooltip content="下一页" describeControl={false}>
            {() => (
              <button
                type="button"
                className="mod-library-pagination__icon-button"
                aria-label="前往下一页"
                aria-disabled={nextDisabled || undefined}
                onClick={() => requestPage(currentPage + 1)}
              >
                <ChevronRight size={16} strokeWidth={2.25} aria-hidden="true" />
              </button>
            )}
          </ModLibraryControlTooltip>
          <ModLibraryControlTooltip content="最后一页" describeControl={false}>
            {() => (
              <button
                type="button"
                className="mod-library-pagination__icon-button"
                aria-label="前往最后一页"
                aria-disabled={nextDisabled || undefined}
                onClick={() => requestPage(totalPages)}
              >
                <ChevronsRight size={16} strokeWidth={2.25} aria-hidden="true" />
              </button>
            )}
          </ModLibraryControlTooltip>
        </nav>

        <div className="mod-library-pagination__range">
          {busy ? (
            <span className="mod-library-pagination__busy" aria-hidden="true">
              <span className="mod-library-pagination__busy-indicator" />
              {/* 文字单独成元素，窄容器下用 display:none 隐藏，不再靠 font-size:0 抹掉。 */}
              <span className="mod-library-pagination__busy-label">更新中</span>
            </span>
          ) : null}
          <span className="mod-library-pagination__range-compact" aria-hidden="true">
            {matchingTotal === 0 ? "0 项" : `${range.start}–${range.end} / ${matchingTotal}`}
          </span>
          <span className="mod-library-pagination__range-live" aria-live="polite" aria-atomic="true">
            {rangeAnnouncement}
          </span>
        </div>
      </div>
    </footer>
  );
}
