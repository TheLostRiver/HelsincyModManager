import { ArrowUp } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { modLibraryCopy } from "./modLibraryCopy";

type BackToTopButtonProps = {
  onClick: () => void;
  visible: boolean;
};

export function BackToTopButton({ onClick, visible }: BackToTopButtonProps) {
  const { locale } = useI18n();
  const label = resolveCopy(modLibraryCopy, locale).backToTop;
  return (
    <button
      type="button"
      className={`mod-library__back-to-top${visible ? " is-visible" : ""}`}
      aria-label={label}
      aria-hidden={!visible}
      tabIndex={visible ? 0 : -1}
      disabled={!visible}
      title={visible ? label : undefined}
      onClick={onClick}
    >
      <ArrowUp size={18} strokeWidth={2} aria-hidden="true" />
    </button>
  );
}
