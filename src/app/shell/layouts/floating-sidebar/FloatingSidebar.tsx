import type { MouseEvent } from "react";
import { AppBrandMark } from "../../../branding/AppBrandMark";
import { useAppRoute } from "../../../routing/useAppRoute";
import { navItems, type NavItem } from "../../navigation/navItems";
import { FloatingSidebarModeButton } from "../../sidebar-mode-control/SidebarModeControl";
import type { NavigationStateItem } from "../../../routing/routeTypes";

export function FloatingSidebar() {
  const { getNavigationState, navigate } = useAppRoute();
  const navigationItems = getNavigationState(navItems);
  const primaryItems = navigationItems.filter((item) => item.placement !== "utility");
  const utilityItems = navigationItems.filter((item) => item.placement === "utility");

  return (
    <aside className="floating-sidebar" aria-label="主导航">
      <div className="floating-sidebar__brand" aria-label="Helsincy Mod Manager">
        <AppBrandMark className="floating-sidebar__brand-mark" />
      </div>

      <nav className="floating-sidebar__nav" data-tour-id="app.navigation">
        {primaryItems.map((item) => (
          <FloatingNavButton key={item.id} item={item} onNavigate={navigate} />
        ))}
      </nav>

      <nav className="floating-sidebar__utility-nav" aria-label="辅助导航">
        {utilityItems.map((item) => (
          <FloatingNavButton key={item.id} item={item} onNavigate={navigate} />
        ))}
      </nav>

      <FloatingSidebarModeButton />
    </aside>
  );
}

function FloatingNavButton({
  item,
  onNavigate,
}: {
  item: NavigationStateItem<NavItem>;
  onNavigate: (route: string) => void;
}) {
  const Icon = item.icon;
  const isActive = item.isActive;
  const isDisabled = item.isDisabled;
  const label = isDisabled && item.disabledReason ? `${item.label}: ${item.disabledReason}` : item.label;
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
