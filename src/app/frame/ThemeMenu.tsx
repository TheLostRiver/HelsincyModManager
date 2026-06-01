import { Check, ChevronDown, Moon, Sun } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { ColorSchemePreference } from "../appearance/colorSchemeTypes";
import { useColorScheme } from "../appearance/useColorScheme";

const themeOptions: {
  preference: ColorSchemePreference;
  label: string;
  icon: "sun" | "moon" | "system";
}[] = [
  { preference: "light", label: "浅色模式", icon: "sun" },
  { preference: "dark", label: "深色模式", icon: "moon" },
  { preference: "system", label: "跟随系统", icon: "system" },
];

export function ThemeMenu() {
  const { effective, preference, setPreference } = useColorScheme();
  const [isOpen, setIsOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const handlePointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };

    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsOpen(false);
      }
    };

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen]);

  const handleTriggerKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === "Escape") {
      setIsOpen(false);
    }
  };

  const handleSelect = (nextPreference: ColorSchemePreference) => {
    setPreference(nextPreference);
    setIsOpen(false);
  };

  const EffectiveIcon = effective === "dark" ? Moon : Sun;

  return (
    <div className="theme-menu" ref={rootRef}>
      <button
        type="button"
        className="theme-menu__trigger"
        aria-label="选择主题模式"
        aria-haspopup="menu"
        aria-expanded={isOpen}
        onClick={() => setIsOpen((current) => !current)}
        onKeyDown={handleTriggerKeyDown}
      >
        <span className={`theme-menu__icon-circle theme-menu__icon-circle--${effective}`}>
          <EffectiveIcon size={14} strokeWidth={2.25} aria-hidden="true" />
        </span>
        <ChevronDown
          size={14}
          className="theme-menu__chevron"
          data-open={isOpen}
          strokeWidth={2.25}
          aria-hidden="true"
        />
      </button>

      {isOpen ? (
        <div className="theme-menu__panel" role="menu" aria-label="主题模式">
          {themeOptions.map((option) => {
            const isSelected = preference === option.preference;

            return (
              <button
                key={option.preference}
                type="button"
                className="theme-menu__item"
                data-selected={isSelected}
                role="menuitemradio"
                aria-checked={isSelected}
                onClick={() => handleSelect(option.preference)}
              >
                <ThemeOptionIcon icon={option.icon} />
                <span className="theme-menu__label">{option.label}</span>
                {isSelected ? (
                  <Check
                    size={14}
                    className="theme-menu__check"
                    strokeWidth={2.4}
                    aria-hidden="true"
                  />
                ) : null}
              </button>
            );
          })}
        </div>
      ) : null}
    </div>
  );
}

function ThemeOptionIcon({ icon }: { icon: "sun" | "moon" | "system" }) {
  if (icon === "system") {
    return (
      <span className="theme-menu__icon-circle theme-menu__icon-circle--system" aria-hidden="true">
        <span className="theme-menu__system-half theme-menu__system-half--light">
          <Sun size={10} strokeWidth={2.2} />
        </span>
        <span className="theme-menu__system-half theme-menu__system-half--dark">
          <Moon size={10} strokeWidth={2.2} />
        </span>
      </span>
    );
  }

  if (icon === "moon") {
    return (
      <span className="theme-menu__icon-circle theme-menu__icon-circle--dark" aria-hidden="true">
        <Moon size={14} strokeWidth={2.25} />
      </span>
    );
  }

  return (
    <span className="theme-menu__icon-circle theme-menu__icon-circle--light" aria-hidden="true">
      <Sun size={14} strokeWidth={2.25} />
    </span>
  );
}
