import { autoUpdate, flip, FloatingFocusManager, FloatingPortal, offset, shift, size, useClick, useDismiss, useFloating, useInteractions, useListNavigation, useRole, useTransitionStyles } from "@floating-ui/react";
import { ArrowDown, ArrowDownWideNarrow, ArrowUp, ArrowUpNarrowWide, Check, ChevronDown, RotateCcw } from "lucide-react";
import { useRef, useState } from "react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { composeModLibrarySort, DEFAULT_MOD_LIBRARY_SORT, MOD_LIBRARY_SORT_FIELDS, modLibrarySortDirection, modLibrarySortField, type ModLibrarySort, type ModLibrarySortDirection } from "./modLibrarySort";
import { modLibrarySortCopy } from "./modLibrarySortCopy";
import "./ModLibrarySortMenu.css";

export function ModLibrarySortMenu({ value, onChange }: { value: ModLibrarySort; onChange: (sort: ModLibrarySort) => void }) {
  const { locale } = useI18n();
  const copy = resolveCopy(modLibrarySortCopy, locale);
  const field = modLibrarySortField(value);
  const direction = modLibrarySortDirection(value);
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState<number | null>(null);
  const listRef = useRef<Array<HTMLElement | null>>([]);
  const floating = useFloating({
    open, onOpenChange: setOpen, placement: "bottom-end", strategy: "fixed", whileElementsMounted: autoUpdate,
    middleware: [offset(8), flip({ padding: 12 }), shift({ padding: 12 }), size({
      padding: 12,
      apply({ availableHeight, elements }) { elements.floating.style.maxHeight = `${Math.max(0, availableHeight)}px`; },
    })],
  });
  const click = useClick(floating.context);
  const dismiss = useDismiss(floating.context);
  const role = useRole(floating.context, { role: "menu" });
  const navigation = useListNavigation(floating.context, { listRef, activeIndex, selectedIndex: MOD_LIBRARY_SORT_FIELDS.indexOf(field), onNavigate: setActiveIndex, loop: true });
  const { getReferenceProps, getFloatingProps, getItemProps } = useInteractions([click, dismiss, role, navigation]);
  const { isMounted, styles } = useTransitionStyles(floating.context, { duration: 120, initial: { opacity: 0 }, open: { opacity: 1 } });
  const choose = (sort: ModLibrarySort, close: boolean) => { if (sort !== value) onChange(sort); if (close) setOpen(false); };
  const DirectionIcon = direction === "asc" ? ArrowUpNarrowWide : ArrowDownWideNarrow;

  return <>
    <button type="button" ref={floating.refs.setReference} className="library-sort-trigger" data-open={open}
      {...getReferenceProps({ "aria-label": `${copy.label}：${copy.options[value]}` })}>
      <DirectionIcon size={16} aria-hidden="true" />
      <span>{copy.fields[field]}</span>
      <ChevronDown size={13} className="library-sort-trigger__chevron" aria-hidden="true" />
    </button>
    {isMounted ? <FloatingPortal>
      <FloatingFocusManager context={floating.context} modal={false}>
        <div ref={floating.refs.setFloating} style={{ ...floating.floatingStyles, ...styles }} className="library-sort-menu"
          {...getFloatingProps({ "aria-label": copy.label })}>
          <div role="group" aria-label={copy.fieldLabel}>
            {MOD_LIBRARY_SORT_FIELDS.map((option, index) => <button key={option} type="button"
              ref={(element) => { listRef.current[index] = element; }} className="library-sort-menu__option"
              role="menuitemradio" aria-checked={option === field} tabIndex={activeIndex === index ? 0 : -1}
              title={option === "size" ? copy.sizeHint : option === "imported_at" ? copy.unknownHint : undefined}
              data-sort-field={option}
              {...getItemProps({ onClick: () => choose(composeModLibrarySort(option, direction), false) })}>
              <span>{copy.fields[option]}</span>
              <Check size={14} aria-hidden="true" />
            </button>)}
          </div>
          <div className="library-sort-menu__directions" role="group" aria-label={copy.directionLabel}>
            {(["asc", "desc"] as const).map((option: ModLibrarySortDirection, offsetIndex) => {
              const index = MOD_LIBRARY_SORT_FIELDS.length + offsetIndex;
              const Icon = option === "asc" ? ArrowUp : ArrowDown;
              return <button key={option} type="button" role="menuitemradio" aria-checked={option === direction}
                className="library-sort-menu__direction" data-sort-direction={option}
                ref={(element) => { listRef.current[index] = element; }} tabIndex={activeIndex === index ? 0 : -1}
                {...getItemProps({
                  onClick: () => choose(composeModLibrarySort(field, option), true),
                  onKeyDown: (event) => {
                    if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
                      event.preventDefault(); event.stopPropagation();
                      const next = event.key === "ArrowLeft" ? 3 : 4;
                      setActiveIndex(next); listRef.current[next]?.focus();
                    }
                  },
                })}>
                <Icon size={13} aria-hidden="true" />{copy.directions[option]}
              </button>;
            })}
          </div>
          <button type="button" className="library-sort-menu__reset" role="menuitem"
            ref={(element) => { listRef.current[5] = element; }} tabIndex={activeIndex === 5 ? 0 : -1}
            aria-disabled={value === DEFAULT_MOD_LIBRARY_SORT || undefined}
            {...getItemProps({ onClick: () => choose(DEFAULT_MOD_LIBRARY_SORT, true) })}>
            <RotateCcw size={13} aria-hidden="true" />{copy.reset}
          </button>
        </div>
      </FloatingFocusManager>
    </FloatingPortal> : null}
  </>;
}
