import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { cwd } from "node:process";
import { test } from "node:test";

const repoRoot = cwd();

function readSource(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

// 只用于"页面无硬编码中文"断言：去掉块注释和纯注释行，但保留含 URL 的代码行
// （naive 的 `//.*` 正则会把 "https://..." 字符串吃掉半截）。
function stripComments(source) {
  const withoutBlocks = source.replace(/\/\*[\s\S]*?\*\//g, "");
  return withoutBlocks
    .split("\n")
    .filter((line) => {
      const trimmed = line.trim();
      return !trimmed.startsWith("//") && !trimmed.startsWith("*");
    })
    .join("\n");
}

test("locale 单一来源：语言自称名只允许定义在 locales.ts", () => {
  const locales = readSource("src/shared/i18n/locales.ts");

  // 自称名是切换 UI 的展示列，收敛在 localeMeta；组件侧只能经 localeMeta 取。
  assert.match(locales, /nativeName: "简体中文"/);
  assert.match(locales, /nativeName: "English"/);
  assert.match(locales, /nativeName: "日本語"/);

  const settingsPage = readSource("src/features/settings/SettingsPage.tsx");
  assert.doesNotMatch(settingsPage, /"简体中文"|"日本語"/, "语言自称名不得硬编码在组件里");
  assert.match(settingsPage, /localeMeta\[systemLocale\]\.nativeName/);
});

test("fallback 链终点统一为 en", () => {
  const locales = readSource("src/shared/i18n/locales.ts");

  assert.match(locales, /zh_cn: \{[^}]*fallback: \["en"\]/);
  assert.match(locales, /ja: \{[^}]*fallback: \["en"\]/);
  assert.match(locales, /en: \{[^}]*fallback: \[\]/);
});

test("I18nProvider 挂载在组合根最外层", () => {
  const app = readSource("src/App.tsx");

  const i18nIndex = app.indexOf("<I18nProvider>");
  const feedbackIndex = app.indexOf("<FeedbackProvider>");
  assert.ok(i18nIndex >= 0, "App.tsx 必须挂载 I18nProvider");
  assert.ok(feedbackIndex >= 0);
  // feedback/toast 等共享层未来也要取词，i18n 必须包在它们外面。
  assert.ok(i18nIndex < feedbackIndex, "I18nProvider 必须在 FeedbackProvider 之外");
});

test("语言偏好持久化：版本化 JSON 且读取带校验", () => {
  const storage = readSource("src/shared/i18n/localeStorage.ts");

  assert.match(storage, /"helsincy\.localePreference"/);
  assert.match(storage, /version: 1/);
  assert.match(storage, /isLocalePreference/);
  // 读失败必须落默认值而不是抛异常（localStorage 可能被禁用）。
  assert.match(storage, /catch \{\s*return defaultLocalePreference;/);
});

test("copy 字典三语齐全并由 satisfies 锁定", () => {
  for (const relativePath of [
    "src/features/settings/settingsPageCopy.ts",
    "src/features/about/aboutPageCopy.ts",
    "src/features/settings/backgroundProtectionCopy.ts",
    "src/features/settings/debugLogSettingsCopy.ts",
    "src/features/game-setup/gamePrerequisiteCopy.ts",
    "src/features/mods/modLibraryCopy.ts",
    "src/features/mods/modImportCopy.ts",
    "src/features/mods/external-import/externalImportCopy.ts",
    "src/features/mods/modDetailDialogCopy.ts",
    "src/features/mods/modLifecycleCopy.ts",
    "src/features/mods/modDeleteCopy.ts",
    "src/features/mods/modReinstallCopy.ts",
    "src/features/mods/batch-lifecycle/batchModLifecycleCopy.ts",
    "src/features/replacements/replacementCopy.ts",
    "src/features/profiles/profilePageCopy.ts",
    "src/features/profiles/profileListCopy.ts",
    "src/features/profiles/saveDirectoryCopy.ts",
    "src/features/profiles/backupPolicyCopy.ts",
    "src/features/profiles/saveBackupCopy.ts",
    "src/features/profiles/saveRestoreCopy.ts",
    "src/features/install-recovery/recoveryCenterCopy.ts",
    "src/features/dashboard/dashboardCopy.ts",
    "src/features/game-setup/gameSetupCopy.ts",
    "src/features/game-launch/gameLaunchCopy.ts",
    "src/features/categories/categoryCopy.ts",
    "src/features/diagnostics/diagnosticsCopy.ts",
    "src/features/backups/backupCenterCopy.ts",
    "src/app/appShellCopy.ts",
    "src/app/onboarding/onboardingTourCopy.ts",
    "src/shared/feedback/feedbackCopy.ts",
    "src/shared/onboarding/tourOverlayCopy.ts",
  ]) {
    const source = readSource(relativePath);
    for (const locale of ["zh_cn", "en", "ja"]) {
      assert.match(
        source,
        new RegExp(`\\b${locale}: \\{`),
        `${relativePath} 缺少 ${locale} 字典`,
      );
    }
    assert.match(
      source,
      /satisfies LocaleDictionary</,
      `${relativePath} 必须用 satisfies LocaleDictionary<T> 锁定三语 key 完备性`,
    );
  }
});

test("试点页与设置页内嵌面板去注释后不再包含硬编码中文文案", () => {
  for (const relativePath of [
    "src/features/settings/SettingsPage.tsx",
    "src/features/about/AboutPage.tsx",
    "src/features/settings/BackgroundProtectionPanel.tsx",
    "src/features/settings/backgroundProtectionTypes.ts",
    "src/features/settings/DebugLogSettingsPanel.tsx",
    "src/features/settings/debugLogSettingsTypes.ts",
    "src/features/game-setup/GamePrerequisitePanel.tsx",
    "src/features/game-setup/gamePrerequisiteViewModel.ts",
    "src/features/mods/ModLibraryPage.tsx",
    "src/features/mods/LibraryToolbar.tsx",
    "src/features/mods/ModLibraryPagination.tsx",
    "src/features/mods/ModLibraryQueryFeedback.tsx",
    "src/features/mods/ModPosterCard.tsx",
    "src/features/mods/ModContextMenu.tsx",
    "src/features/mods/BackToTopButton.tsx",
    "src/features/mods/CompactActionPanel.tsx",
    "src/features/mods/compactActionAvailability.ts",
    "src/features/mods/ModImportAction.tsx",
    "src/features/mods/modImportTaskState.ts",
    "src/features/mods/modLibraryFilters.ts",
    "src/features/mods/modSelection.ts",
    "src/features/mods/modLibraryQueryState.ts",
    "src/features/mods/external-import/ExternalImportAction.tsx",
    "src/features/mods/external-import/ExternalImportCandidateSelectionItem.tsx",
    "src/features/mods/external-import/ExternalImportSelectionPanel.tsx",
    "src/features/mods/external-import/ExternalImportResultPanel.tsx",
    "src/features/mods/external-import/externalImportScanState.ts",
    "src/features/mods/external-import/externalImportProgressState.ts",
    "src/features/mods/external-import/externalImportSelectionModel.ts",
    "src/features/mods/external-import/externalImportResultModel.ts",
    "src/features/mods/external-import/externalImportPreviewModel.ts",
    "src/features/mods/external-import/useExternalImportResultWorkflow.ts",
    "src/features/mods/external-import/useExternalImportSelectionWorkflow.ts",
    "src/features/mods/external-import/useExternalImportTaskProgress.ts",
    "src/features/mods/ModDetailDialog.tsx",
    "src/features/mods/ModLifecycleFeedback.tsx",
    "src/features/mods/modLifecycleFeedbackState.ts",
    "src/features/mods/modInstallTaskState.ts",
    "src/features/mods/modPrerequisiteDecision.ts",
    "src/features/mods/ReinstallPlanPreviewPanel.tsx",
    "src/features/mods/useModReinstallWorkflow.ts",
    "src/features/mods/modReinstallTaskState.ts",
    "src/features/mods/batch-lifecycle/BatchModLifecyclePreviewPanel.tsx",
    "src/features/mods/batch-lifecycle/BatchModLifecycleResultPanel.tsx",
    "src/features/replacements/ReplacementTargetPanel.tsx",
    "src/features/replacements/replacementWorkflow.ts",
    "src/features/replacements/replacementErrorText.ts",
    "src/features/profiles/ProfilePage.tsx",
    "src/features/profiles/ProfileListPanel.tsx",
    "src/features/profiles/SaveDirectoryPanel.tsx",
    "src/features/profiles/ProfileSaveDirectoryCandidateList.tsx",
    "src/features/profiles/ProfileSaveDirectoryDiscoveryProvider.tsx",
    "src/features/profiles/BackupPolicyPanel.tsx",
    "src/features/profiles/BackupSchedulePicker.tsx",
    "src/features/profiles/SaveRestoreDialog.tsx",
    "src/features/profiles/ActiveProfileProvider.tsx",
    "src/features/profiles/profileViewModel.ts",
    "src/features/profiles/profileApi.ts",
    "src/features/profiles/profileSaveBackupApi.ts",
    "src/features/profiles/profileSaveBackupTaskState.ts",
    "src/features/profiles/profileSaveBackupTypes.ts",
    "src/features/profiles/profileSaveDirectoryDiscoveryApi.ts",
    "src/features/profiles/profileSaveDirectoryDiscoveryTypes.ts",
    "src/features/profiles/profileSaveRestoreApi.ts",
    "src/features/profiles/profileSaveRestoreTaskState.ts",
    "src/features/profiles/profileSaveRestoreTypes.ts",
    "src/features/profiles/profileSaveSettingsApi.ts",
    "src/features/profiles/profileSaveSettingsDefaults.ts",
    "src/features/profiles/profileSaveSettingsTypes.ts",
    "src/features/profiles/profileTypes.ts",
    "src/features/install-recovery/RecoveryCenterPage.tsx",
    "src/features/install-recovery/InstallRecoveryGlobalAlertPanel.tsx",
    "src/features/install-recovery/recoveryCenterViewModel.ts",
    "src/features/install-recovery/installRecoveryGlobalAlert.ts",
    "src/features/install-recovery/installRecoveryHealth.ts",
    "src/features/install-recovery/installRecoveryRefresh.ts",
    "src/features/install-recovery/recoveryCenterManualActions.ts",
    "src/features/install-recovery/recoveryDiagnosticsApi.ts",
    "src/features/install-recovery/recoveryDiagnosticsTypes.ts",
    "src/features/install-recovery/useInstallRecoveryHealth.ts",
    "src/features/install-recovery/useRecoveryCenterScan.ts",
    "src/features/install-recovery/useRecoveryDiagnosticsExport.ts",
    "src/features/install-recovery/useRecoveryRollback.ts",
    "src/features/dashboard/DashboardPage.tsx",
    "src/features/dashboard/DashboardHeroCard.tsx",
    "src/features/dashboard/DashboardModulePreview.tsx",
    "src/features/dashboard/SetupStatusPanel.tsx",
    "src/features/dashboard/InstallRecoveryHealthPanel.tsx",
    "src/features/dashboard/dashboardData.ts",
    "src/features/dashboard/setupStatusSteps.ts",
    "src/features/game-setup/GameDirectoryActions.tsx",
    "src/features/game-setup/GameDirectoryCandidateList.tsx",
    "src/features/game-setup/GameSetupProvider.tsx",
    "src/features/game-setup/gameSetupApi.ts",
    "src/features/game-setup/gameSetupTypes.ts",
    "src/features/game-setup/gameSetupViewModel.ts",
    "src/features/game-setup/useGameSetup.ts",
    "src/features/game-setup/useGamePrerequisites.ts",
    "src/features/game-setup/gamePrerequisiteApi.ts",
    "src/features/game-setup/gamePrerequisiteTypes.ts",
    "src/features/game-launch/useGameLaunch.ts",
    "src/features/game-launch/gameLaunchApi.ts",
    "src/features/game-launch/gameLaunchTypes.ts",
    "src/features/categories/CategoryPage.tsx",
    "src/features/categories/CategoryList.tsx",
    "src/features/categories/CategoryColorPicker.tsx",
    "src/features/categories/CategorySortMenu.tsx",
    "src/features/categories/categoryApi.ts",
    "src/features/categories/categoryWorkflow.ts",
    "src/features/categories/useCategoryList.ts",
    "src/features/diagnostics/DiagnosticsPage.tsx",
    "src/features/diagnostics/diagnosticsApi.ts",
    "src/features/diagnostics/diagnosticsTypes.ts",
    "src/features/diagnostics/diagnosticsPageLogic.ts",
    "src/features/backups/BackupCenterPage.tsx",
    "src/features/backups/backupCenterApi.ts",
    "src/features/backups/backupCenterTypes.ts",
    "src/app/frame/AppHeader.tsx",
    "src/app/frame/ThemeMenu.tsx",
    "src/app/shell/navigation/navItems.ts",
    "src/app/shell/layouts/classic-sidebar/ClassicSidebar.tsx",
    "src/app/shell/layouts/floating-sidebar/FloatingSidebar.tsx",
    "src/app/shell/sidebar-mode-control/SidebarModeControl.tsx",
    "src/app/window-lifecycle/WindowCloseDialog.tsx",
    "src/app/window-lifecycle/WindowCloseDialogHost.tsx",
    "src/app/window-lifecycle/windowLifecycleError.ts",
    "src/app/window-lifecycle/useWindowCloseRequest.ts",
    "src/app/onboarding/firstRunTour.ts",
    "src/app/onboarding/TourProvider.tsx",
    "src/shared/feedback/FeedbackToast.tsx",
    "src/shared/feedback/ModalSurface.tsx",
    "src/shared/feedback/TaskNoticeViewport.tsx",
    "src/shared/feedback/ToastViewport.tsx",
    "src/shared/onboarding/TourOverlay.tsx",
  ]) {
    const source = stripComments(readSource(relativePath));
    const hanMatches = source.match(/[一-鿿][^\n]{0,40}/gu) ?? [];
    assert.deepEqual(
      hanMatches,
      [],
      `${relativePath} 仍有硬编码中文：${hanMatches.slice(0, 3).join(" | ")}`,
    );
  }
});

test("设置页语言切换：跟随系统置顶且切换反馈用切换后的语言", () => {
  const settingsPage = readSource("src/features/settings/SettingsPage.tsx");

  const systemOptionIndex = settingsPage.indexOf('value: "system"');
  const coreLocalesIndex = settingsPage.indexOf("coreLocales.map");
  assert.ok(systemOptionIndex >= 0, "语言选项必须包含跟随系统");
  assert.ok(coreLocalesIndex >= 0, "语言选项必须由 coreLocales 生成，不得手写枚举");
  assert.ok(systemOptionIndex < coreLocalesIndex, "跟随系统必须排在语言列表最前");

  // 用户选了日语就不该收到中文 toast：反馈文案取自切换后的 locale。
  assert.match(settingsPage, /const nextLocale = value === "system" \? systemLocale : value;/);
  assert.match(settingsPage, /resolveCopy\(settingsPageCopy, nextLocale\)/);
});
