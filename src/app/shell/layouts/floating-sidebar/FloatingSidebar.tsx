import type { MouseEvent } from "react";
import { AppBrandMark } from "../../../branding/AppBrandMark";
import { resolveCopy, useI18n } from "../../../../shared/i18n";
import { useAppRoute } from "../../../routing/useAppRoute";
import { appShellCopy, type AppShellCopy } from "../../../appShellCopy";
import { navItems, type NavItem } from "../../navigation/navItems";
import { FloatingSidebarModeButton } from "../../sidebar-mode-control/SidebarModeControl";
import type { NavigationStateItem } from "../../../routing/routeTypes";

export function FloatingSidebar() {
  const { locale } = useI18n();
  const copy = resolveCopy(appShellCopy, locale);
  const { getNavigationState, navigate } = useAppRoute();
  const navigationItems = getNavigationState(navItems);
  const primaryItems = navigationItems.filter((item) => item.placement !== "utility");
  const utilityItems = navigationItems.filter((item) => item.placement === "utility");

  return (
    <aside className="floating-sidebar" aria-label={copy.sidebar.primaryAria}>
      <div className="floating-sidebar__brand" aria-label="Helsincy Mod Manager">
        <AppBrandMark className="floating-sidebar__brand-mark" />
      </div>

      <nav className="floating-sidebar__nav" data-tour-id="app.navigation">
        {primaryItems.map((item) => (
          <FloatingNavButton key={item.id} copy={copy} item={item} onNavigate={navigate} />
        ))}
      </nav>

      <nav className="floating-sidebar__utility-nav" aria-label={copy.sidebar.utilityAria}>
        {utilityItems.map((item) => (
          <FloatingNavButton key={item.id} copy={copy} item={item} onNavigate={navigate} />
        ))}
      </nav>

      <FloatingSidebarModeButton />
    </aside>
  );
}

function FloatingNavButton({
  copy,
  item,
  onNavigate,
}: {
  copy: AppShellCopy;
  item: NavigationStateItem<NavItem>;
  onNavigate: (route: string) => void;
}) {
  const Icon = item.icon;
  const isActive = item.isActive;
  const isDisabled = item.isDisabled;
  const baseLabel = copy.nav.labels[item.id];
  const disabledReason = copy.nav.disabledReasons[item.id];
  const label = isDisabled && disabledReason ? `${baseLabel}: ${disabledReason}` : baseLabel;
  const handleClick = (event: MouseEvent<HTMLButtonElement>) => {
    if (isDisabled) {
      event.preventDefault();
      event.stopPropagation();
      return;
    }

    onNavigate(item.route);
  };

  return (
    <button
      type="button"
      className={`floating-sidebar__item ${isActive ? "is-active" : ""}`}
      data-tour-id={`nav.${item.id}`}
      aria-disabled={isDisabled || undefined}
      aria-current={isActive ? "page" : undefined}
      aria-label={label}
      title={label}
      onClick={handleClick}
    >
      <Icon size={18} strokeWidth={2.1} />
    </button>
  );
}
