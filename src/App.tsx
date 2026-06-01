import { AppShell } from "./app/AppShell";
import { ColorSchemeProvider } from "./app/appearance/ColorSchemeProvider";
import { SidebarModeProvider } from "./app/shell/SidebarModeProvider";
import { DashboardPage } from "./features/dashboard/DashboardPage";

export function App() {
  return (
    <ColorSchemeProvider>
      <SidebarModeProvider>
        <AppShell>
          <DashboardPage />
        </AppShell>
      </SidebarModeProvider>
    </ColorSchemeProvider>
  );
}
