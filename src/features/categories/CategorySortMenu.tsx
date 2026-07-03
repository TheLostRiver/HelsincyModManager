import { ArrowDownWideNarrow, Check, ChevronDown } from "lucide-react";
import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import type { CategorySortMode } from "./categoryWorkflow";

export type CategorySortOption = {
  value: CategorySortMode;
  label: string;
};

type CategorySortMenuProps = {
  value: CategorySortMode;
  options: readonly CategorySortOption[];
  onChange: (value: CategorySortMode) => void;
};

export function CategorySortMenu({ value, options, onChange }: CategorySortMenuProps) {
  const [open, setOpen] = useState(false);
  const menuId = useId();
  const rootRef = useRef<HTMLDivElement | null>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const selectedIndex = Math.max(0, options.findIndex((option) => option.value === value));
  const selectedOption = options[selectedIndex] ?? options[0];

  useEffect(() => {
    if (!open) {
      return;
    }

    const handlePointerDown = (event: PointerEvent) => {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
        triggerRef.current?.focus();
      }
    };

    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

  const selectOption = (nextValue: CategorySortMode) => {
    onChange(nextValue);
    setOpen(false);
    triggerRef.current?.focus();
  };

  const moveSelection = (offset: -1 | 1) => {
    if (options.length === 0) {
      return;
    }

    const nextIndex = (selectedIndex + offset + options.length) % options.length;
    onChange(options[nextIndex].value);
  };

  const handleTriggerKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        setOpen(true);
        moveSelection(1);
        break;
      case "ArrowUp":
        event.preventDefault();
        setOpen(true);
        moveSelection(-1);
        break;
      case "Home":
        event.preventDefault();
        setOpen(true);
        if (options[0]) {
          onChange(options[0].value);
        }
        break;
      case "End":
        event.preventDefault();
        setOpen(true);
        if (options[options.length - 1]) {
          onChange(options[options.length - 1].value);
        }
        break;
      case "Escape":
        setOpen(false);
        break;
    }
  };

  return (
    <div className="category-sort-menu" ref={rootRef}>
      <button
        type="button"
        ref={triggerRef}
        className={`category-sort-menu__trigger ${open ? "is-open" : ""}`}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={menuId}
        aria-label="排序视图"
        onClick={() => setOpen((current) => !current)}
        onKeyDown={handleTriggerKeyDown}
      >
        <ArrowDownWideNarrow size={15} strokeWidth={2.2} aria-hidden="true" />
        <span>{selectedOption?.label ?? "排序"}</span>
        <ChevronDown
          size={15}
          strokeWidth={2.2}
          className="category-sort-menu__chevron"
          aria-hidden="true"
        />
      </button>

      {open && (
        <div className="category-sort-menu__popover" id={menuId} role="listbox" aria-label="排序视图">
          {options.map((option) => {
            const selected = option.value === value;

            return (
              <button
                key={option.value}
                type="button"
                className={`category-sort-menu__option ${selected ? "is-selected" : ""}`}
                role="option"
                aria-selected={selected}
                onClick={() => selectOption(option.value)}
              >
                <span>{option.label}</span>
                {selected && <Check size={14} strokeWidth={2.5} aria-hidden="true" />}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
