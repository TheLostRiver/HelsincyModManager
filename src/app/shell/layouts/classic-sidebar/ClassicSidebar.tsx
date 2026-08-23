import { AppBrandMark } from "../../../branding/AppBrandMark";
import { resolveCopy, useI18n } from "../../../../shared/i18n";
import { useAppRoute } from "../../../routing/useAppRoute";
import type { NavigationStateItem } from "../../../routing/routeTypes";
import { appShellCopy, type AppShellCopy } from "../../../appShellCopy";
import { navItems, type NavItem } from "../../navigation/navItems";
import { ClassicSidebarModeButton } from "../../sidebar-mode-control/SidebarModeControl";

export function ClassicSidebar() {
  const { locale } = useI18n();
  const copy = resolveCopy(appShellCopy, locale);
  const { getNavigationState, navigate } = useAppRoute();
  const navigationItems = getNavigationState(navItems);
  const primaryItems = navigationItems.filter((item) => item.placement !== "utility");
  const utilityItems = navigationItems.filter((item) => item.placement === "utility");

  return (
    <aside className="sidebar" aria-label={copy.sidebar.primaryAria}>
      <div className="brand-block">
        <AppBrandMark className="brand-block__mark" />
        <div className="brand-block__copy">
          <h1>Helsincy</h1>
          <p>Mod Manager</p>
        </div>
      </div>

      <nav className="nav-list" data-tour-id="app.navigation">
        {primaryItems.map((item) => (
          <ClassicNavButton key={item.id} copy={copy} item={item} onNavigate={navigate} />
        ))}
      </nav>

      <div className="sidebar-footer">
        <nav className="sidebar-utility-nav" aria-label={copy.sidebar.utilityAria}>
          {utilityItems.map((item) => (
            <ClassicNavButton key={item.id} copy={copy} item={item} onNavigate={navigate} />
          ))}
        </nav>
        <ClassicSidebarModeButton />

        <div className="nav-footnote">
          <span aria-hidden="true" />
          <p>MHW:I&nbsp;&nbsp;{copy.sidebar.footnote}</p>
        </div>
      </div>
    </aside>
  );
}

function ClassicNavButton({
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

  return (
    <button
      type="button"
      className={`nav-item ${isActive ? "is-active" : ""}`}
      data-tour-id={`nav.${item.id}`}
      disabled={isDisabled}
      aria-current={isActive ? "page" : undefined}
      title={isDisabled ? copy.nav.disabledReasons[item.id] : undefined}
      onClick={() => onNavigate(item.route)}
    >
      {isActive && <span className="active-mark" aria-hidden="true" />}
      <Icon size={16} strokeWidth={2.1} />
      <span>{copy.nav.labels[item.id]}</span>
    </button>
  );
}
