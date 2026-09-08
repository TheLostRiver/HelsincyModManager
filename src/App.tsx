import { AppShell } from "./app/AppShell";
import { ColorSchemeProvider } from "./app/appearance/ColorSchemeProvider";
import { TourProvider } from "./app/onboarding/TourProvider";
import { AppRouteProvider } from "./app/routing/AppRouteProvider";
import { RouterOutlet } from "./app/routing/RouterOutlet";
import { SidebarModeProvider } from "./app/shell/SidebarModeProvider";
import { GameSetupProvider } from "./features/game-setup/GameSetupProvider";
import { InstallConfigTargetProvider } from "./features/install-config/InstallConfigTargetProvider";
import { ExternalStateSessionProvider } from "./features/mods/ExternalStateSessionProvider";
import { ModImportDropProvider } from "./features/mods/ModImportDropProvider";
import { ActiveProfileProvider } from "./features/profiles/ActiveProfileProvider";
import { ProfileSaveDirectoryDiscoveryProvider } from "./features/profiles/ProfileSaveDirectoryDiscoveryProvider";
import { ModStorageSettingsProvider } from "./features/settings/ModStorageSettingsProvider";
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
                        {/* #275：存储目录快照与迁移进度要活过路由切换，且库页要读同一份
                            writesFrozen 来禁用导入 / 删除入口。 */}
                        <ModStorageSettingsProvider>
                          {/* #354 D4：安装配置是覆盖层不是路由。挂在这一层是因为它由 Provider
                              自己渲染面板（保证同时只有一个），而面板走 FeedbackPortal，
                              必须在 FeedbackProvider 之内。 */}
                          <InstallConfigTargetProvider>
                            {/* T22 / #366：拖拽导入也要活过路由切换。它自己渲染清单浮层，
                                而队列必须比页面活得久——RouterOutlet 会卸载页面，挂在页面里
                                的话切个页就把批量循环打断，剩下的包**永远不会起且不报错**。 */}
                            <ModImportDropProvider>
                              <AppShell>
                                <RouterOutlet />
                              </AppShell>
                            </ModImportDropProvider>
                          </InstallConfigTargetProvider>
                        </ModStorageSettingsProvider>
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
