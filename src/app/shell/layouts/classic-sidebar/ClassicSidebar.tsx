import { AppBrandMark } from "../../../branding/AppBrandMark";
import { useAppRoute } from "../../../routing/useAppRoute";
import type { NavigationStateItem } from "../../../routing/routeTypes";
import { navItems, type NavItem } from "../../navigation/navItems";
import { ClassicSidebarModeButton } from "../../sidebar-mode-control/SidebarModeControl";

export function ClassicSidebar() {
  const { getNavigationState, navigate } = useAppRoute();
  const navigationItems = getNavigationState(navItems);

  return (
    <aside className="sidebar" aria-label="主导航">
      <div className="brand-block">
        <AppBrandMark className="brand-block__mark" />
        <div className="brand-block__copy">
          <h1>Helsincy</h1>
          <p>Mod Manager</p>
        </div>
      </div>

      <nav className="nav-list" data-tour-id="app.navigation">
        {navigationItems.map((item) => (
          <ClassicNavButton key={item.id} item={item} onNavigate={navigate} />
        ))}
      </nav>

      <div className="sidebar-footer">
        <ClassicSidebarModeButton />

        <div className="nav-footnote">
          <span aria-hidden="true" />
          <p>MHW:I&nbsp;&nbsp;首次启动</p>
        </div>
      </div>
    </aside>
  );
}

function ClassicNavButton({
  item,
  onNavigate,
}: {
  item: NavigationStateItem<NavItem>;
  onNavigate: (route: string) => void;
}) {
  const Icon = item.icon;
  const isActive = item.isActive;
  const isDisabled = item.isDisabled;

  return (
    <button
      type="button"
      className={`nav-item ${isActive ? "is-active" : ""}`}
      data-tour-id={`nav.${item.id}`}
      disabled={isDisabled}
      aria-current={isActive ? "page" : undefined}
      title={isDisabled ? item.disabledReason : undefined}
      onClick={() => onNavigate(item.route)}
    >
      {isActive && <span className="active-mark" aria-hidden="true" />}
      <Icon size={16} strokeWidth={2.1} />
      <span>{item.label}</span>
    </button>
  );
}
