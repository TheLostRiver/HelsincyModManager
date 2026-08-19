import { AppShell } from "./app/AppShell";
import { ColorSchemeProvider } from "./app/appearance/ColorSchemeProvider";
import { TourProvider } from "./app/onboarding/TourProvider";
import { AppRouteProvider } from "./app/routing/AppRouteProvider";
import { RouterOutlet } from "./app/routing/RouterOutlet";
import { SidebarModeProvider } from "./app/shell/SidebarModeProvider";
import { GameSetupProvider } from "./features/game-setup/GameSetupProvider";
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
              {/* 游戏目录状态要在 AppShell 之上：顶部状态栏和各页面必须读同一份，
                  否则在工作台配置完目录，顶栏仍显示未配置。 */}
              <GameSetupProvider>
                <ActiveProfileProvider>
                  <ProfileSaveDirectoryDiscoveryProvider>
                    <AppShell>
                      <RouterOutlet />
                    </AppShell>
                  </ProfileSaveDirectoryDiscoveryProvider>
                </ActiveProfileProvider>
              </GameSetupProvider>
            </TourProvider>
          </AppRouteProvider>
        </SidebarModeProvider>
      </ColorSchemeProvider>
    </FeedbackProvider>
  );
}
