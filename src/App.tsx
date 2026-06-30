import { AppShell } from "./app/AppShell";
import { ColorSchemeProvider } from "./app/appearance/ColorSchemeProvider";
import { AppRouteProvider } from "./app/routing/AppRouteProvider";
import { RouterOutlet } from "./app/routing/RouterOutlet";
import { SidebarModeProvider } from "./app/shell/SidebarModeProvider";
import { ActiveProfileProvider } from "./features/profiles/ActiveProfileProvider";

export function App() {
  return (
    <ColorSchemeProvider>
      <SidebarModeProvider>
        <AppRouteProvider>
          <ActiveProfileProvider>
            <AppShell>
              <RouterOutlet />
            </AppShell>
          </ActiveProfileProvider>
        </AppRouteProvider>
      </SidebarModeProvider>
    </ColorSchemeProvider>
  );
}
