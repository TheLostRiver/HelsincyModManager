import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./shared/styles/reset.css";
import "./shared/styles/tokens.css";
import "./app/frame/AppFrame.css";
import "./app/frame/ThemeMenu.css";
import "./app/routing/RouterOutlet.css";
import "./app/shell/sidebar-mode-control/SidebarModeControl.css";
import "./app/shell/layouts/classic-sidebar/ClassicSidebar.css";
import "./app/shell/layouts/floating-sidebar/FloatingSidebar.css";
import "./features/dashboard/Dashboard.css";
import "./features/game-setup/GamePrerequisitePanel.css";
import "./features/install-recovery/RecoveryCenterPage.css";
import "./features/mods/ModLibraryPage.css";
import "./features/mods/ModLibraryPaginationLayout.css";
import "./features/settings/SettingsPage.css";
import "./features/categories/CategoryPage.css";
import "./features/profiles/ProfilePage.css";
import "./features/profiles/ProfileSaveManager.css";
import "./features/profiles/ProfileSaveDirectoryDiscovery.css";
import "./features/backups/BackupCenterPage.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
