import { ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight } from "lucide-react";
import {
  getModLibraryEllipsisTarget,
  getModLibraryPageSlots,
  getModLibraryTotalPages,
  type ModLibraryPageSize,
} from "./modLibraryPaginationModel";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { modLibraryCopy } from "./modLibraryCopy";
import { ModLibraryControlTooltip } from "./ModLibraryControlTooltip";
import "./ModLibraryPagination.css";

export type ModLibraryPaginationProps = {
  page: number;
  pageSize: ModLibraryPageSize;
  matchingTotal: number;
  busy?: boolean;
  onPageChange: (page: number) => void;
};

export function ModLibraryPagination({
  page, pageSize, matchingTotal, busy = false, onPageChange,
}: ModLibraryPaginationProps) {
  const { locale } = useI18n();
  const pagination = resolveCopy(modLibraryCopy, locale).pagination;
  const totalPages = getModLibraryTotalPages(matchingTotal, pageSize);
  if (totalPages <= 1) return null;

  const currentPage = Math.min(Math.max(1, Math.floor(page)), totalPages);
  const pageSlots = getModLibraryPageSlots(currentPage, totalPages);
  const requestPage = (nextPage: number) => {
    if (busy || nextPage < 1 || nextPage > totalPages || nextPage === currentPage) return;
    onPageChange(nextPage);
  };
  const previousDisabled = busy || currentPage <= 1;
  const nextDisabled = busy || currentPage >= totalPages;

  return (
    <footer className="mod-library-pagination" aria-label={pagination.toolbarAria}>
      <div className="mod-library-pagination__layout">
        <nav
          className="mod-library-pagination__navigation"
          aria-label={pagination.pageNavAria}
          aria-busy={busy}
        >
          <ModLibraryControlTooltip content={pagination.firstPage} describeControl={false}>
            {() => (
              <button
                type="button"
                className="mod-library-pagination__icon-button"
                aria-label={pagination.gotoFirst}
                aria-disabled={previousDisabled || undefined}
                onClick={() => requestPage(1)}
              >
                <ChevronsLeft size={16} strokeWidth={2.25} aria-hidden="true" />
              </button>
            )}
          </ModLibraryControlTooltip>
          <ModLibraryControlTooltip content={pagination.prevPage} describeControl={false}>
            {() => (
              <button
                type="button"
                className="mod-library-pagination__icon-button"
                aria-label={pagination.gotoPrev}
                aria-disabled={previousDisabled || undefined}
                onClick={() => requestPage(currentPage - 1)}
              >
                <ChevronLeft size={16} strokeWidth={2.25} aria-hidden="true" />
              </button>
            )}
          </ModLibraryControlTooltip>

          <div className="mod-library-pagination__page-list" aria-label={pagination.pageListAria}>
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
                      content={pagination.jumpTo(target)}
                      describeControl={false}
                    >
                      {() => (
                        <button
                          type="button"
                          className="mod-library-pagination__ellipsis is-interactive"
                          aria-label={pagination.jumpTo(target)}
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
                  aria-label={pagination.pageAria(slot)}
                  aria-current={slot === currentPage ? "page" : undefined}
                  aria-disabled={busy || undefined}
                  onClick={() => requestPage(slot)}
                >
                  {slot}
                </button>
              ),
            )}
          </div>

          <ModLibraryControlTooltip content={pagination.nextPage} describeControl={false}>
            {() => (
              <button
                type="button"
                className="mod-library-pagination__icon-button"
                aria-label={pagination.gotoNext}
                aria-disabled={nextDisabled || undefined}
                onClick={() => requestPage(currentPage + 1)}
              >
                <ChevronRight size={16} strokeWidth={2.25} aria-hidden="true" />
              </button>
            )}
          </ModLibraryControlTooltip>
          <ModLibraryControlTooltip content={pagination.lastPage} describeControl={false}>
            {() => (
              <button
                type="button"
                className="mod-library-pagination__icon-button"
                aria-label={pagination.gotoLast}
                aria-disabled={nextDisabled || undefined}
                onClick={() => requestPage(totalPages)}
              >
                <ChevronsRight size={16} strokeWidth={2.25} aria-hidden="true" />
              </button>
            )}
          </ModLibraryControlTooltip>
        </nav>
      </div>
    </footer>
  );
}
