import { AlertTriangle, PackageOpen, RefreshCw, SearchX, SlidersHorizontal } from "lucide-react";
import type { ModViewMode } from "./ModLibraryPage";
import { ModLibraryControlTooltip } from "./ModLibraryControlTooltip";
import "./ModLibraryQueryFeedback.css";

type ModLibrarySkeletonProps = {
  viewMode: ModViewMode;
};

export function ModLibrarySkeleton({ viewMode }: ModLibrarySkeletonProps) {
  const itemCount = viewMode === "classic" || viewMode === "grid" ? 8 : 6;

  return (
    <div
      className={`mod-library-skeleton view-${viewMode}`}
      role="status"
      aria-label="正在加载 Mod 库"
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
  return (
    <div className="mod-library-state" role="alert">
      <span className="mod-library-state__icon is-error" aria-hidden="true">
        <AlertTriangle size={22} strokeWidth={2.1} />
      </span>
      <strong>Mod 库暂时不可用</strong>
      <p>{message}</p>
      <button type="button" className="mod-library-state__action" onClick={onRetry}>
        <RefreshCw size={15} aria-hidden="true" />
        重试
      </button>
    </div>
  );
}

type ModLibraryQueryBlockedStateProps = {
  message: string;
  onReset: () => void;
};

export function ModLibraryQueryBlockedState({ message, onReset }: ModLibraryQueryBlockedStateProps) {
  return (
    <div className="mod-library-state" role="status">
      <span className="mod-library-state__icon" aria-hidden="true">
        <SlidersHorizontal size={22} strokeWidth={2.1} />
      </span>
      <strong>当前筛选暂不可用</strong>
      <p>{message}</p>
      <button type="button" className="mod-library-state__action" onClick={onReset}>
        查看全部 Mod
      </button>
    </div>
  );
}

type ModLibraryEmptyStateProps =
  | { kind: "library" }
  | { kind: "matches"; onReset: () => void };

export function ModLibraryEmptyState(props: ModLibraryEmptyStateProps) {
  if (props.kind === "library") {
    return (
      <div className="mod-library-state" role="status">
        <span className="mod-library-state__icon" aria-hidden="true">
          <PackageOpen size={23} strokeWidth={2} />
        </span>
        <strong>尚未导入 Mod</strong>
        <p>Mod 库当前为空。</p>
      </div>
    );
  }

  return (
    <div className="mod-library-state" role="status">
      <span className="mod-library-state__icon" aria-hidden="true">
        <SearchX size={23} strokeWidth={2} />
      </span>
      <strong>没有匹配的 Mod</strong>
      <p>当前搜索与筛选条件没有结果。</p>
      <button type="button" className="mod-library-state__action" onClick={props.onReset}>
        <SlidersHorizontal size={15} aria-hidden="true" />
        清除条件
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
  return (
    <>
      {busy ? (
        <div className="mod-library-query-progress" role="status" aria-label="正在更新 Mod 列表">
          <span aria-hidden="true" />
        </div>
      ) : null}

      {errorMessage ? (
        <div className="mod-library-query-error" role="alert">
          <AlertTriangle size={16} strokeWidth={2.2} aria-hidden="true" />
          <span className="mod-library-query-error__message">{errorMessage}</span>
          <ModLibraryControlTooltip content="重试 Mod 库查询" describeControl={false}>
            {() => (
              <button type="button" onClick={onRetry} aria-label="重试 Mod 库查询">
                <RefreshCw size={15} aria-hidden="true" />
              </button>
            )}
          </ModLibraryControlTooltip>
        </div>
      ) : null}
    </>
  );
}
