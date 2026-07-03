import { Check, Paintbrush } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";

const DEFAULT_PICKER_COLOR = "#3B82F6";

export const CATEGORY_COLOR_OPTIONS = [
  { label: "蓝色", value: "#2563EB" },
  { label: "青色", value: "#0891B2" },
  { label: "绿色", value: "#16A34A" },
  { label: "琥珀", value: "#D97706" },
  { label: "红色", value: "#DC2626" },
  { label: "粉色", value: "#DB2777" },
  { label: "紫色", value: "#7C3AED" },
  { label: "灰色", value: "#64748B" },
];

export function isValidColor(value: string): boolean {
  return /^#(?:[0-9a-fA-F]{3}){1,2}$/.test(value.trim());
}

export function isFullHexColor(value: string): boolean {
  return /^#[0-9a-fA-F]{6}$/.test(value.trim());
}

type CategoryColorPickerProps = {
  value: string;
  onChange: (value: string) => void;
  /** 触发按钮上除颜色状态外的说明，用于区分页面里多个色板入口。 */
  triggerLabel?: string;
  align?: "start" | "end";
};

export function CategoryColorPicker({
  value,
  onChange,
  triggerLabel,
  align = "start",
}: CategoryColorPickerProps) {
  const [open, setOpen] = useState(false);
  const popoverId = useId();
  const rootRef = useRef<HTMLDivElement | null>(null);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const selectedColor = isValidColor(value) ? value.trim() : "";
  const nativePickerColor = isFullHexColor(selectedColor) ? selectedColor : DEFAULT_PICKER_COLOR;

  useEffect(() => {
    if (!open) {
      return;
    }

    const handlePointerDown = (event: PointerEvent) => {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
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

  const selectColor = (color: string) => {
    onChange(color);
    setOpen(false);
    triggerRef.current?.focus();
  };

  return (
    <div className={`category-color-picker ${align === "end" ? "is-align-end" : ""}`} ref={rootRef}>
      <button
        type="button"
        ref={triggerRef}
        className="category-color-picker__trigger"
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={popoverId}
        aria-label={triggerLabel}
        onClick={() => setOpen((current) => !current)}
      >
        <span
          className="category-swatch"
          style={{ background: selectedColor || undefined }}
          aria-hidden="true"
        />
        <span>{selectedColor ? selectedColor.toUpperCase() : "默认颜色"}</span>
      </button>

      {open && (
        <div className="category-color-popover" id={popoverId} role="dialog" aria-label="选择分类颜色">
          <div className="category-color-palette" aria-label="常用颜色">
            {CATEGORY_COLOR_OPTIONS.map((option) => {
              const isSelected = selectedColor.toLowerCase() === option.value.toLowerCase();

              return (
                <button
                  type="button"
                  className={`category-color-swatch-button ${isSelected ? "is-selected" : ""}`}
                  key={option.value}
                  onClick={() => selectColor(option.value)}
                  aria-label={`选择${option.label}`}
                  aria-pressed={isSelected}
                  title={option.label}
                >
                  <span
                    className="category-swatch"
                    style={{ background: option.value }}
                    aria-hidden="true"
                  />
                  {isSelected && <Check size={12} strokeWidth={3} aria-hidden="true" />}
                </button>
              );
            })}
          </div>
          <label className="category-custom-color">
            <span>
              <Paintbrush size={13} strokeWidth={2.2} aria-hidden="true" />
              自定义
            </span>
            <input
              type="color"
              value={nativePickerColor}
              onChange={(event) => onChange(event.target.value)}
              aria-label="自定义颜色"
            />
          </label>
          <button type="button" className="category-color-clear" onClick={() => selectColor("")}>
            恢复默认颜色
          </button>
        </div>
      )}
    </div>
  );
}
