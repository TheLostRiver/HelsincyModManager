import type { ReactNode } from "react";
import { AppHeader } from "./frame/AppHeader";
import { navItems, type NavItem } from "./shell/navigation/navItems";

type AppShellProps = {
  children: ReactNode;
};

export function AppShell({ children }: AppShellProps) {
  return (
    <div className="app-shell">
      <aside className="sidebar" aria-label="主导航">
        <div className="brand-block">
          <h1>Helsincy</h1>
          <p>Mod Manager</p>
        </div>

        <nav className="nav-list">
          {navItems.map((item) => (
            <NavButton key={item.label} item={item} />
          ))}
        </nav>

        <div className="nav-footnote">
          <span aria-hidden="true" />
          <p>MHW:I&nbsp;&nbsp;首次启动</p>
        </div>
      </aside>

      <div className="app-surface">
        <AppHeader />
        <main className="workbench-body">{children}</main>
      </div>
    </div>
  );
}

function NavButton({ item }: { item: NavItem }) {
  const Icon = item.icon;
  const isActive = item.state === "active";

  return (
    <button
      type="button"
      className={`nav-item ${isActive ? "is-active" : ""}`}
      disabled={item.state === "disabled"}
      aria-current={isActive ? "page" : undefined}
    >
      {isActive && <span className="active-mark" aria-hidden="true" />}
      <Icon size={16} strokeWidth={2.1} />
      <span>{item.label}</span>
    </button>
  );
}
