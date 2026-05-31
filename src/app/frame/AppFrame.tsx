import type { ReactNode } from "react";
import { AppHeader } from "./AppHeader";
import { ClassicSidebar } from "../shell/layouts/classic-sidebar/ClassicSidebar";

type AppFrameProps = {
  children: ReactNode;
};

export function AppFrame({ children }: AppFrameProps) {
  return (
    <div className="app-shell" data-sidebar-mode="classic">
      <ClassicSidebar />
      <div className="app-surface">
        <AppHeader />
        <main className="workbench-body">{children}</main>
      </div>
    </div>
  );
}
