import { useEffect, useState } from "react";
import { LoaderCircle, RefreshCw, Target } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import type { GameId } from "../game-setup/gameSetupTypes";
import { getModReplacementSummary } from "./replacementApi";
import { replacementCopy } from "./replacementCopy";
import { replacementIdentityLabel, replacementKindLabel } from "./replacementIdentityLabel";
import { buildReplacementTargetOptions } from "./replacementTargetOptions";
import type { ModReplacementSummary, ReplacementSummaryItem, ReplacementTarget } from "./replacementTypes";
import "./ReplacementContextPanel.css";

type ContextState =
  | { status: "loading"; scope: string }
  | { status: "ready"; scope: string; summary: ModReplacementSummary }
  | { status: "failed"; scope: string };

export function ReplacementContextPanel({ gameId, modId, profileId, reloadKey, targets, onRetry }: {
  gameId: GameId;
  modId: string;
  profileId: string | null;
  reloadKey: number;
  targets: readonly ReplacementTarget[];
  onRetry: () => void;
}) {
  const { locale } = useI18n();
  const copy = resolveCopy(replacementCopy, locale).panel;
  const scope = JSON.stringify([gameId, modId, profileId, reloadKey]);
  const [state, setState] = useState<ContextState>({ status: "loading", scope });

  useEffect(() => {
    let disposed = false;
    setState({ status: "loading", scope });
    void Promise.resolve().then(() => disposed ? null : getModReplacementSummary({ gameId, modId, profileId }))
      .then((summary) => {
        if (disposed || summary === null) return;
        if (summary.gameId !== gameId || summary.modId !== modId) {
          setState({ status: "failed", scope });
          return;
        }
        setState({ status: "ready", scope, summary });
      })
      .catch(() => { if (!disposed) setState({ status: "failed", scope }); });
    return () => { disposed = true; };
  }, [gameId, modId, profileId, scope]);

  if (state.scope !== scope || state.status === "loading") {
    return <section className="replacement-context" role="status">
      <LoaderCircle size={17} className="replacement-panel__spinner" aria-hidden="true" />
      <span>{copy.contextLoading}</span>
    </section>;
  }
  if (state.status === "failed") {
    return <section className="replacement-context" role="status">
      <p>{copy.contextUnavailable}</p>
      <button type="button" onClick={onRetry}><RefreshCw size={15} aria-hidden="true" />{copy.retry}</button>
    </section>;
  }

  const renderItems = (items: ReplacementSummaryItem[]) => items.length === 0
    ? <p className="replacement-panel__empty">{copy.noSources}</p>
    : <dl className="replacement-context__items">{items.map((item) => {
      // 只按后端提供的同一 ID 补充共享名称，不在前端猜编号、path family 或默认对象。
      const catalogTarget = targets.find((target) => target.id === item.id && target.gameId === gameId
        && target.targetType === item.kind && target.internalId === item.internalId);
      const named = Object.keys(item.displayNames).length > 0;
      const sharedNames = named && catalogTarget?.targetType === "weapon"
        ? buildReplacementTargetOptions([catalogTarget], locale).slice(1).map((option) => option.displayName)
        : [];
      return <div key={item.id}>
        <dt>{replacementKindLabel(item.kind, locale)}</dt>
        <dd>
          <strong>{replacementIdentityLabel(item, locale)}</strong>
          {!named ? <small>{copy.contextNameUnknown}</small> : null}
          {sharedNames.length > 0 ? <div className="replacement-context__shared">
            <small>{copy.contextSharedNames}</small>
            <ul>{sharedNames.map((name) => <li key={name}>{name}</li>)}</ul>
          </div> : null}
        </dd>
      </div>;
    })}</dl>;
  const { sources, installedTargets } = state.summary;
  return <section className="replacement-context">
    {installedTargets !== null && installedTargets.length > 0 ? (
      <div className="replacement-context__current">
        <h3><Target size={17} aria-hidden="true" />{copy.currentTargetsTitle}</h3>
        <p>{copy.currentTargetsHint}</p>
        {renderItems(installedTargets)}
      </div>
    ) : null}
    {profileId !== null && installedTargets === null ? <p className="replacement-context__unknown" role="status">{copy.currentTargetsUnknown}</p> : null}
    <div className="replacement-context__default">
      <h3><Target size={17} aria-hidden="true" />{copy.defaultTargetsTitle}</h3>
      <p>{copy.defaultTargetsHint}</p>
      {renderItems(sources)}
    </div>
  </section>;
}
