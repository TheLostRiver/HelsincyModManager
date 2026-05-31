import type { MouseEvent } from "react";
import { navItems, type NavItem } from "../../navigation/navItems";
import { FloatingSidebarModeButton } from "../../sidebar-mode-control/SidebarModeControl";

export function FloatingSidebar() {
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

      <FloatingSidebarModeButton />
    </aside>
  );
}

function FloatingNavButton({ item }: { item: NavItem }) {
  const Icon = item.icon;
  const isActive = item.state === "active";
  const isDisabled = item.state === "disabled";
  const label = isDisabled && item.disabledReason ? `${item.label}: ${item.disabledReason}` : item.label;
  const handleClick = (event: MouseEvent<HTMLButtonElement>) => {
    if (isDisabled) {
      event.preventDefault();
      event.stopPropagation();
    }
  };

  return (
    <button
      type="button"
      className={`floating-sidebar__item ${isActive ? "is-active" : ""}`}
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
