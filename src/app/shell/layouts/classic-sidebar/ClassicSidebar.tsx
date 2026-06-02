import { navItems, type NavItem } from "../../navigation/navItems";
import { ClassicSidebarModeButton } from "../../sidebar-mode-control/SidebarModeControl";

export function ClassicSidebar() {
  return (
    <aside className="sidebar" aria-label="主导航">
      <div className="brand-block">
        <h1>Helsincy</h1>
        <p>Mod Manager</p>
      </div>

      <nav className="nav-list">
        {navItems.map((item) => (
          <ClassicNavButton key={item.id} item={item} />
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

function ClassicNavButton({ item }: { item: NavItem }) {
  const Icon = item.icon;
  const isActive = item.state === "active";
  const isDisabled = item.state === "disabled";

  return (
    <button
      type="button"
      className={`nav-item ${isActive ? "is-active" : ""}`}
      disabled={isDisabled}
      aria-current={isActive ? "page" : undefined}
      title={isDisabled ? item.disabledReason : undefined}
    >
      {isActive && <span className="active-mark" aria-hidden="true" />}
      <Icon size={16} strokeWidth={2.1} />
      <span>{item.label}</span>
    </button>
  );
}
