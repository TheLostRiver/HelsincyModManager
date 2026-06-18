import { AppShell } from "./app/AppShell";
import { ColorSchemeProvider } from "./app/appearance/ColorSchemeProvider";
import { AppRouteProvider } from "./app/routing/AppRouteProvider";
import { RouterOutlet } from "./app/routing/RouterOutlet";
import { SidebarModeProvider } from "./app/shell/SidebarModeProvider";

export function App() {
  return (
    <ColorSchemeProvider>
      <SidebarModeProvider>
        <AppRouteProvider>
          <AppShell>
            <RouterOutlet />
          </AppShell>
        </AppRouteProvider>
      </SidebarModeProvider>
    </ColorSchemeProvider>
  );
}
