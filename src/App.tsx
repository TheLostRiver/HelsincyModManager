import { AppShell } from "./app/AppShell";
import { ColorSchemeProvider } from "./app/appearance/ColorSchemeProvider";
import { TourProvider } from "./app/onboarding/TourProvider";
import { AppRouteProvider } from "./app/routing/AppRouteProvider";
import { RouterOutlet } from "./app/routing/RouterOutlet";
import { SidebarModeProvider } from "./app/shell/SidebarModeProvider";
import { ActiveProfileProvider } from "./features/profiles/ActiveProfileProvider";
import { ProfileSaveDirectoryDiscoveryProvider } from "./features/profiles/ProfileSaveDirectoryDiscoveryProvider";
import { FeedbackProvider } from "./shared/feedback";

export function App() {
  return (
    <FeedbackProvider>
      <ColorSchemeProvider>
        <SidebarModeProvider>
          <AppRouteProvider>
            <TourProvider>
              <ActiveProfileProvider>
                <ProfileSaveDirectoryDiscoveryProvider>
                  <AppShell>
                    <RouterOutlet />
                  </AppShell>
                </ProfileSaveDirectoryDiscoveryProvider>
              </ActiveProfileProvider>
            </TourProvider>
          </AppRouteProvider>
        </SidebarModeProvider>
      </ColorSchemeProvider>
    </FeedbackProvider>
  );
}
