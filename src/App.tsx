import { AppShell } from "./app/AppShell";
import { ColorSchemeProvider } from "./app/appearance/ColorSchemeProvider";
import { AppRouteProvider } from "./app/routing/AppRouteProvider";
import { RouterOutlet } from "./app/routing/RouterOutlet";
import { SidebarModeProvider } from "./app/shell/SidebarModeProvider";
import { ActiveProfileProvider } from "./features/profiles/ActiveProfileProvider";
import { ProfileSaveDirectoryDiscoveryProvider } from "./features/profiles/ProfileSaveDirectoryDiscoveryProvider";

export function App() {
  return (
    <ColorSchemeProvider>
      <SidebarModeProvider>
        <AppRouteProvider>
          <ActiveProfileProvider>
            <ProfileSaveDirectoryDiscoveryProvider>
              <AppShell>
                <RouterOutlet />
              </AppShell>
            </ProfileSaveDirectoryDiscoveryProvider>
          </ActiveProfileProvider>
        </AppRouteProvider>
      </SidebarModeProvider>
    </ColorSchemeProvider>
  );
}
