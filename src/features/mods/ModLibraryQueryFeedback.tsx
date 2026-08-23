import { AlertTriangle, PackageOpen, RefreshCw, SearchX, SlidersHorizontal } from "lucide-react";
import type { ModViewMode } from "./ModLibraryPage";
import { ModLibraryControlTooltip } from "./ModLibraryControlTooltip";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { modLibraryCopy } from "./modLibraryCopy";
import "./ModLibraryQueryFeedback.css";

type ModLibrarySkeletonProps = {
  viewMode: ModViewMode;
};

export function ModLibrarySkeleton({ viewMode }: ModLibrarySkeletonProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(modLibraryCopy, locale).queryFeedback;
  const itemCount = viewMode === "classic" || viewMode === "grid" ? 8 : 6;

  return (
    <div
      className={`mod-library-skeleton view-${viewMode}`}
      role="status"
      aria-label={copy.loadingAria}
    >
      {Array.from({ length: itemCount }, (_, index) => (
        <div className="mod-library-skeleton__item" key={index} aria-hidden="true">
          {viewMode === "tech" ? null : <span className="mod-library-skeleton__poster" />}
          <span className="mod-library-skeleton__body">
            <span className="mod-library-skeleton__line is-title" />
            <span className="mod-library-skeleton__line is-meta" />
            <span className="mod-library-skeleton__line is-short" />
          </span>
        </div>
      ))}
    </div>
  );
}

type ModLibraryInitialErrorProps = {
  message: string;
  onRetry: () => void;
};

export function ModLibraryInitialError({ message, onRetry }: ModLibraryInitialErrorProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(modLibraryCopy, locale).queryFeedback;
  return (
    <div className="mod-library-state" role="alert">
      <span className="mod-library-state__icon is-error" aria-hidden="true">
        <AlertTriangle size={22} strokeWidth={2.1} />
      </span>
      <strong>{copy.unavailableTitle}</strong>
      <p>{message}</p>
      <button type="button" className="mod-library-state__action" onClick={onRetry}>
        <RefreshCw size={15} aria-hidden="true" />
        {copy.retry}
      </button>
    </div>
  );
}

type ModLibraryQueryBlockedStateProps = {
  message: string;
  onReset: () => void;
};

export function ModLibraryQueryBlockedState({ message, onReset }: ModLibraryQueryBlockedStateProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(modLibraryCopy, locale).queryFeedback;
  return (
    <div className="mod-library-state" role="status">
      <span className="mod-library-state__icon" aria-hidden="true">
        <SlidersHorizontal size={22} strokeWidth={2.1} />
      </span>
      <strong>{copy.filterUnavailableTitle}</strong>
      <p>{message}</p>
      <button type="button" className="mod-library-state__action" onClick={onReset}>
        {copy.viewAllMods}
      </button>
    </div>
  );
}

type ModLibraryEmptyStateProps =
  | { kind: "library" }
  | { kind: "matches"; onReset: () => void };

export function ModLibraryEmptyState(props: ModLibraryEmptyStateProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(modLibraryCopy, locale).queryFeedback;
  if (props.kind === "library") {
    return (
      <div className="mod-library-state" role="status">
        <span className="mod-library-state__icon" aria-hidden="true">
          <PackageOpen size={23} strokeWidth={2} />
        </span>
        <strong>{copy.emptyTitle}</strong>
        <p>{copy.emptyBody}</p>
      </div>
    );
  }

  return (
    <div className="mod-library-state" role="status">
      <span className="mod-library-state__icon" aria-hidden="true">
        <SearchX size={23} strokeWidth={2} />
      </span>
      <strong>{copy.noMatchTitle}</strong>
      <p>{copy.noMatchBody}</p>
      <button type="button" className="mod-library-state__action" onClick={props.onReset}>
        <SlidersHorizontal size={15} aria-hidden="true" />
        {copy.clearFilters}
      </button>
    </div>
  );
}

type ModLibraryQueryFeedbackProps = {
  busy: boolean;
  errorMessage: string | null;
  onRetry: () => void;
};

export function ModLibraryQueryFeedback({ busy, errorMessage, onRetry }: ModLibraryQueryFeedbackProps) {
  const { locale } = useI18n();
  const copy = resolveCopy(modLibraryCopy, locale).queryFeedback;
  return (
    <>
      {busy ? (
        <div className="mod-library-query-progress" role="status" aria-label={copy.updatingAria}>
          <span aria-hidden="true" />
        </div>
      ) : null}

      {errorMessage ? (
        <div className="mod-library-query-error" role="alert">
          <AlertTriangle size={16} strokeWidth={2.2} aria-hidden="true" />
          <span className="mod-library-query-error__message">{errorMessage}</span>
          <ModLibraryControlTooltip content={copy.retryQueryAria} describeControl={false}>
            {() => (
              <button type="button" onClick={onRetry} aria-label={copy.retryQueryAria}>
                <RefreshCw size={15} aria-hidden="true" />
              </button>
            )}
          </ModLibraryControlTooltip>
        </div>
      ) : null}
    </>
  );
}
