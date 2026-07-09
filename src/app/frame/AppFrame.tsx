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
        <AppHeader />
        <InstallRecoveryGlobalAlert />
        <main className="workbench-body">{children}</main>
      </div>
      <WindowCloseDialogHost />
    </div>
  );
}
