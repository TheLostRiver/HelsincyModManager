import type { ReactNode } from "react";
import { AppHeader } from "./frame/AppHeader";
import { ClassicSidebar } from "./shell/layouts/classic-sidebar/ClassicSidebar";

type AppShellProps = {
  children: ReactNode;
};

export function AppShell({ children }: AppShellProps) {
  return (
    <div className="app-shell">
      <ClassicSidebar />
      <div className="app-surface">
        <AppHeader />
        <main className="workbench-body">{children}</main>
      </div>
    </div>
  );
}
