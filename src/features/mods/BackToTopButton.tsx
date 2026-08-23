import { ArrowUp } from "lucide-react";
import { resolveCopy, useI18n } from "../../shared/i18n";
import { modLibraryCopy } from "./modLibraryCopy";

type BackToTopButtonProps = {
  onClick: () => void;
};

export function BackToTopButton({ onClick }: BackToTopButtonProps) {
  const { locale } = useI18n();
  const label = resolveCopy(modLibraryCopy, locale).backToTop;
  return (
    <button
      type="button"
      className="mod-library__back-to-top"
      aria-label={label}
      title={label}
      onClick={onClick}
    >
      <ArrowUp size={20} strokeWidth={2.5} aria-hidden="true" />
    </button>
  );
}
