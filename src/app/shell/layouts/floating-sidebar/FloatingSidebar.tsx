import { PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { navItems, type NavItem } from "../../navigation/navItems";
import { useSidebarMode } from "../../useSidebarMode";

export function FloatingSidebar() {
  const { toggleSidebarMode } = useSidebarMode();

  return (
    <aside className="floating-sidebar" aria-label="主导航">
      <div className="floating-sidebar__brand" aria-label="Helsincy">
        H
      </div>

      <nav className="floating-sidebar__nav">
        {navItems.map((item) => (
          <FloatingNavButton key={item.id} item={item} />
        ))}
      </nav>

      <button
        type="button"
        className="floating-sidebar__mode-button"
        aria-label="切换为普通侧边栏"
        title="切换为普通侧边栏"
        onClick={toggleSidebarMode}
      >
        <PanelLeftOpen size={18} strokeWidth={2.1} />
      </button>
    </aside>
  );
}

function FloatingNavButton({ item }: { item: NavItem }) {
  const Icon = item.icon;
  const isActive = item.state === "active";
  const isDisabled = item.state === "disabled";
  const label = isDisabled && item.disabledReason ? `${item.label}：${item.disabledReason}` : item.label;

  return (
    <button
      type="button"
      className={`floating-sidebar__item ${isActive ? "is-active" : ""}`}
      disabled={isDisabled}
      aria-current={isActive ? "page" : undefined}
      aria-label={label}
      title={label}
    >
      <Icon size={18} strokeWidth={2.1} />
    </button>
  );
}

export function ClassicSidebarModeButton() {
  const { toggleSidebarMode } = useSidebarMode();

  return (
    <button type="button" className="sidebar-mode-button" aria-label="切换为悬浮侧边栏" onClick={toggleSidebarMode}>
      <PanelLeftClose size={16} strokeWidth={2.1} />
      <span>悬浮侧边栏</span>
    </button>
  );
}
