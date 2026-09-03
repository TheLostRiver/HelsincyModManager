import { AppShell } from "./app/AppShell";
import { ColorSchemeProvider } from "./app/appearance/ColorSchemeProvider";
import { TourProvider } from "./app/onboarding/TourProvider";
import { AppRouteProvider } from "./app/routing/AppRouteProvider";
import { RouterOutlet } from "./app/routing/RouterOutlet";
import { SidebarModeProvider } from "./app/shell/SidebarModeProvider";
import { GameSetupProvider } from "./features/game-setup/GameSetupProvider";
import { ExternalStateSessionProvider } from "./features/mods/ExternalStateSessionProvider";
import { ActiveProfileProvider } from "./features/profiles/ActiveProfileProvider";
import { ProfileSaveDirectoryDiscoveryProvider } from "./features/profiles/ProfileSaveDirectoryDiscoveryProvider";
import { FeedbackProvider } from "./shared/feedback";
import { I18nProvider } from "./shared/i18n";

export function App() {
  return (
    // I18nProvider 在最外层：feedback/toast 等共享层未来也要取词。
    <I18nProvider>
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
                      {/* #286 3b-2 A+：外部状态扫描结果的会话表要活过路由切换，
                          RouterOutlet 会卸载页面，所以表挂在它之上。 */}
                      <ExternalStateSessionProvider>
                        <AppShell>
                          <RouterOutlet />
                        </AppShell>
                      </ExternalStateSessionProvider>
                    </ProfileSaveDirectoryDiscoveryProvider>
                  </ActiveProfileProvider>
                </GameSetupProvider>
              </TourProvider>
            </AppRouteProvider>
          </SidebarModeProvider>
        </ColorSchemeProvider>
      </FeedbackProvider>
    </I18nProvider>
  );
}
