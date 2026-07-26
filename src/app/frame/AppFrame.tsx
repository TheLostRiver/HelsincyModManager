import type { ReactNode } from "react";
import { ClassicSidebar } from "../shell/layouts/classic-sidebar/ClassicSidebar";
import { FloatingSidebar } from "../shell/layouts/floating-sidebar/FloatingSidebar";
import { useSidebarMode } from "../shell/useSidebarMode";
import { AppHeader } from "./AppHeader";
import { WindowCloseDialogHost } from "../window-lifecycle/WindowCloseDialogHost";
import { InstallRecoveryGlobalAlert } from "../../features/install-recovery/InstallRecoveryGlobalAlertPanel";

type AppFrameProps = {
  children: ReactNode;
};

export function AppFrame({ children }: AppFrameProps) {
  const { sidebarMode } = useSidebarMode();
  const Sidebar = sidebarMode === "floating" ? FloatingSidebar : ClassicSidebar;

  return (
    <div className="app-shell" data-sidebar-mode={sidebarMode}>
      <Sidebar />
      <div className="app-surface">
        {/*
         * 状态栏本身是带圆角和阴影的悬浮卡片，背景只覆盖圆角矩形，无法遮住身后滚过的内容。
         * 因此把吸顶职责提到这层满幅背板上，由背板负责不透明遮挡与页面留白。
         */}
        <div className="app-surface__header-dock">
          <AppHeader />
        </div>
        <InstallRecoveryGlobalAlert />
        <main className="workbench-body">{children}</main>
      </div>
      <WindowCloseDialogHost />
    </div>
  );
}
