import { PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { resolveCopy, useI18n } from "../../../shared/i18n";
import { appShellCopy } from "../../appShellCopy";
import { useSidebarMode } from "../useSidebarMode";

export function FloatingSidebarModeButton() {
  const { locale } = useI18n();
  const copy = resolveCopy(appShellCopy, locale).sidebar;
  const { toggleSidebarMode } = useSidebarMode();

  return (
    <button
      type="button"
      className="floating-sidebar__mode-button"
      aria-label={copy.switchToClassic}
      title={copy.switchToClassic}
      onClick={toggleSidebarMode}
    >
      <PanelLeftOpen size={18} strokeWidth={2.1} />
    </button>
  );
}

export function ClassicSidebarModeButton() {
  const { locale } = useI18n();
  const copy = resolveCopy(appShellCopy, locale).sidebar;
  const { toggleSidebarMode } = useSidebarMode();

  return (
    <button type="button" className="sidebar-mode-button" aria-label={copy.switchToFloating} onClick={toggleSidebarMode}>
      <PanelLeftClose size={16} strokeWidth={2.1} />
      <span>{copy.floatingModeLabel}</span>
    </button>
  );
}
