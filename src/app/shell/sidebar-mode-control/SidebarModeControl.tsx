import { PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { useSidebarMode } from "../useSidebarMode";

export function FloatingSidebarModeButton() {
  const { toggleSidebarMode } = useSidebarMode();

  return (
    <button
      type="button"
      className="floating-sidebar__mode-button"
      aria-label="切换为普通侧边栏"
      title="切换为普通侧边栏"
      onClick={toggleSidebarMode}
    >
      <PanelLeftOpen size={18} strokeWidth={2.1} />
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
