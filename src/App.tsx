import { AppShell } from "./app/AppShell";
import { SidebarModeProvider } from "./app/shell/SidebarModeProvider";
import { DashboardPage } from "./features/dashboard/DashboardPage";

export function App() {
  return (
    <SidebarModeProvider>
      <AppShell>
        <DashboardPage />
      </AppShell>
    </SidebarModeProvider>
  );
}
