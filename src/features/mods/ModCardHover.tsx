import { autoUpdate, flip, FloatingPortal, offset, safePolygon, shift, useDismiss, useFloating, useFocus, useHover, useInteractions, useRole } from "@floating-ui/react";
import { cloneElement, useState, type ComponentPropsWithRef, type FocusEvent, type ReactElement } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import type { GameId } from "../game-setup/gameSetupTypes";
import { replacementIdentityLabel, replacementKindLabel } from "../replacements/replacementIdentityLabel";
import { replacementSummaryCopy } from "../replacements/replacementSummaryCopy";
import type { ReplacementSummaryItem } from "../replacements/replacementTypes";
import { modHoverCopy } from "./modHoverCopy";
import { modHoverModel } from "./modHoverModel";
import type { ModLibraryItem } from "./modLibraryTypes";
import { modOriginLabel } from "./modOriginView";
import { useModHoverDetails } from "./useModHoverDetails";
import "./ModCardHover.css";

type ModCardHoverProps = {
  item: ModLibraryItem;
  gameId: GameId;
  profileId: string | null;
  children: ReactElement<ComponentPropsWithRef<"div">>;
};

export function ModCardHover({ children, item, gameId, profileId }: ModCardHoverProps) {
  const [open, setOpen] = useState(false);
  const floating = useFloating({ open, onOpenChange: setOpen, placement: "right-start", strategy: "fixed", whileElementsMounted: autoUpdate,
    middleware: [offset(10), flip({ padding: 12, fallbackAxisSideDirection: "end" }), shift({ padding: 12, crossAxis: true })] });
  const hover = useHover(floating.context, { delay: { open: 350, close: 120 }, handleClose: safePolygon() });
  const focus = useFocus(floating.context);
  const dismiss = useDismiss(floating.context, { referencePress: true });
  const role = useRole(floating.context, { role: "tooltip" });
  const { getReferenceProps, getFloatingProps } = useInteractions([hover, focus, dismiss, role]);
  return (
    <>
      {cloneElement(children, { ...getReferenceProps({ ...children.props, onFocus(event: FocusEvent<HTMLDivElement>) {
        children.props.onFocus?.(event);
        // Escape 可能在鼠标已移入浮层后触发；随后真正的键盘焦点必须能重新打开。
        if (event.currentTarget.matches(":focus-visible")) setOpen(true);
      } }), ref: floating.refs.setReference })}
      {open ? (
        <FloatingPortal>
          <aside ref={floating.refs.setFloating} style={floating.floatingStyles} {...getFloatingProps()} className="mod-hover-card">
            <ModCardHoverContent key={JSON.stringify([item.id, gameId, profileId])} item={item} gameId={gameId} profileId={profileId} />
          </aside>
        </FloatingPortal>
      ) : null}
    </>
  );
}

function ModCardHoverContent({ item, gameId, profileId }: Omit<ModCardHoverProps, "children">) {
  const { locale } = useI18n();
  const copy = resolveCopy(modHoverCopy, locale);
  const replacementCopy = resolveCopy(replacementSummaryCopy, locale);
  const data = useModHoverDetails(item.id, gameId, profileId);
  const model = data.detail ? modHoverModel(data.detail, item) : null;
  const fields = model && data.detail ? [
    ["author", copy.author, model.author], ["version", copy.version, model.version], ["notes", copy.notes, model.notes],
    ["categories", copy.categories, model.categories.join(", ")], ["tags", copy.tags, model.tags.join(", ")],
    ["origin", copy.origin, modOriginLabel(data.detail.origin, locale)], ["nexusId", copy.nexusId, model.nexusId],
  ] : [];
  return (
    <>
      <h3>{model?.name ?? item.name}</h3>
      {data.status === "loading" ? <p role="status">{copy.loading}</p> : null}
      {data.status === "unavailable" ? <p role="status">{copy.unavailable}</p> : null}
      {model ? (
        <>
          <dl className="mod-hover-card__metadata">
            {fields.map(([key, label, value]) => <div key={key}><dt>{label}</dt><dd className={key === "notes" ? "mod-hover-card__notes" : undefined}>{value || copy.notProvided}</dd></div>)}
          </dl>
          <div className="mod-hover-card__replacements">
            {data.replacementLoading ? <p role="status">{copy.loading}</p> : data.replacement ? (
              <>
                <h4>{replacementCopy.source}</h4>
                <EquipmentList items={data.replacement.sources} empty={replacementCopy.none} />
                <h4>{replacementCopy.installed}</h4>
                {data.replacement.installedTargets === null ? <p>{profileId ? replacementCopy.unavailable : replacementCopy.profileRequired}</p>
                  : <EquipmentList items={data.replacement.installedTargets} empty={replacementCopy.noBinding} />}
              </>
            ) : <p>{replacementCopy.unavailable}</p>}
          </div>
        </>
      ) : null}
    </>
  );
}

function EquipmentList({ items, empty }: { items: ReplacementSummaryItem[]; empty: string }) {
  const { locale } = useI18n();
  return items.length ? (
    <ul>{items.map((item) => <li key={item.id}><small>{replacementKindLabel(item.kind, locale)}</small><span>{replacementIdentityLabel(item, locale)}</span></li>)}</ul>
  ) : <p>{empty}</p>;
}
