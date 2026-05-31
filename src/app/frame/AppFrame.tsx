import type { ReactNode } from "react";
import { ClassicSidebar } from "../shell/layouts/classic-sidebar/ClassicSidebar";
import { useSidebarMode } from "../shell/useSidebarMode";
import { AppHeader } from "./AppHeader";

type AppFrameProps = {
  children: ReactNode;
};

export function AppFrame({ children }: AppFrameProps) {
  const { sidebarMode } = useSidebarMode();

  return (
    <div className="app-shell" data-sidebar-mode={sidebarMode}>
      <ClassicSidebar />
      <div className="app-surface">
        <AppHeader />
        <main className="workbench-body">{children}</main>
      </div>
    </div>
  );
}
