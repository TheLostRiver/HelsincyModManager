import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import ts from "typescript";

const repoRoot = process.cwd();

function readProjectFile(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

async function importTypeScriptModule(relativePath) {
  const source = readProjectFile(relativePath);
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2022,
    },
    fileName: relativePath,
  });
  const dataUrl = `data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`;
  return import(dataUrl);
}

test("task tour remains an independent route-driven overlay", () => {
  const appSource = readProjectFile("src/App.tsx");
  const headerSource = readProjectFile("src/app/frame/AppHeader.tsx");
  const providerSource = readProjectFile("src/app/onboarding/TourProvider.tsx");
  const overlaySource = readProjectFile("src/shared/onboarding/TourOverlay.tsx");
  const tourSources = [
    providerSource,
    overlaySource,
    readProjectFile("src/app/onboarding/firstRunTour.ts"),
    readProjectFile("src/shared/onboarding/useTourTarget.ts"),
    readProjectFile("src/shared/onboarding/tourTarget.ts"),
  ].join("\n");

  assert.match(appSource, /<AppRouteProvider>[\s\S]*?<TourProvider>[\s\S]*?<AppShell>/);
  assert.match(providerSource, /currentRoute\.id !== "dashboard"/);
  assert.match(providerSource, /buildOnboardingTour\(currentRoute\.id\)/);
  assert.match(providerSource, /shouldAutoStartTour\(firstRunTour, storage\)/);
  assert.match(providerSource, /activeStep\.advance\.expectedRouteId !== currentRoute\.id/);
  assert.match(
    providerSource,
    /requestAnimationFrame\(\(\) => \{\s*autoStartCheckedRef\.current = true;/,
  );
  const statusActionsIndex = headerSource.indexOf('className="status-actions"');
  const launcherIndex = headerSource.indexOf('className="onboarding-launcher"');
  const profileStatusIndex = headerSource.indexOf('className={`status-pill ${activeProfileTone}`}');
  const windowToolsIndex = headerSource.indexOf('className="window-tools"');
  assert.ok(statusActionsIndex < launcherIndex);
  assert.ok(launcherIndex < profileStatusIndex);
  assert.ok(profileStatusIndex < windowToolsIndex);
  assert.match(headerSource, /aria-label="打开新手引导"/);
  assert.match(headerSource, /<span>新手引导<\/span>/);
  assert.match(headerSource, /onClick=\{startTour\}/);
  assert.match(overlaySource, /function InteractionBlockers/);
  assert.match(overlaySource, /interaction !== "target-only"/);
  assert.match(overlaySource, /className="tour-layer__blocker is-top"/);
  assert.match(overlaySource, /className="tour-layer__blocker is-bottom"/);
  assert.match(overlaySource, /useModalFocusTrap\(/);
  assert.match(overlaySource, /active: phase !== "closing" && step\.interaction === "blocked"/);
  assert.match(overlaySource, /aria-live="polite"/);
  assert.match(
    overlaySource,
    /requestAnimationFrame\(\(\) => primaryActionRef\.current\?\.focus\(\)\)/,
  );
  assert.match(
    overlaySource,
    /requestAnimationFrame\(\(\) => targetState\.element\?\.focus\(\)\)/,
  );
  assert.match(overlaySource, /event\.key !== "Tab"/);
  assert.match(overlaySource, /allowedFocusTargets/);
  assert.match(
    overlaySource,
    /event\.key === "Enter" && event\.target === primaryActionRef\.current/,
  );
  assert.match(overlaySource, /requestFinish\("skipped"\)/);
  assert.doesNotMatch(tourSources, /\.click\s*\(/);
  assert.doesNotMatch(tourSources, /onScanSteam|onDirectorySelected|onLaunchGame|invoke\s*</);
});

test("contextual tour rotates from the current page and excludes unavailable routes", async () => {
  const { buildOnboardingTour, ONBOARDING_ROUTE_ORDER, rotateRoutesFrom } =
    await importTypeScriptModule("src/app/onboarding/firstRunTour.ts");
  const routeRegistry = readProjectFile("src/app/routing/routeRegistry.tsx");
  const registeredRouteIds = [...routeRegistry.matchAll(/id:\s*"([^"]+)"/g)].map((match) => match[1]);

  assert.deepEqual(
    new Set(ONBOARDING_ROUTE_ORDER),
    new Set(registeredRouteIds.filter((routeId) => routeId !== "categories")),
  );
  assert.equal(ONBOARDING_ROUTE_ORDER.includes("categories"), false);
  assert.deepEqual(rotateRoutesFrom("profiles"), [
    "profiles",
    "backups",
    "diagnostics",
    "settings",
    "dashboard",
    "mods",
    "recovery",
  ]);

  const manualTour = buildOnboardingTour("profiles");
  assert.equal(manualTour.contentVersion, 4);
  assert.equal(manualTour.steps[0].id, "page-profiles");
  assert.equal(manualTour.steps[0].target, "page.profiles");
  assert.equal(manualTour.steps[1].id, "profiles-list");
  assert.equal(manualTour.steps.length, 39);
  assert.equal(manualTour.steps.filter((step) => step.interaction === "target-only").length, 6);
  assert.deepEqual(
    manualTour.steps.find((step) => step.id === "profiles-directories"),
    {
      id: "profiles-directories",
      title: "设置存档与备份目录",
      description: "游戏存档是需要保护的源目录，备份目录是 HMM 存放归档包和清单的位置。",
      target: "profiles.save-directories",
      fallbackTarget: "profiles.settings",
      placement: "left-start",
      bullets: ["可以自动检测存档目录，也可以手动选择。", "两个目录都通过校验后再保存设置。"],
      primaryLabel: "继续",
      spotlightPadding: 6,
      interaction: "blocked",
      advance: { kind: "controls" },
    },
  );
  for (const step of manualTour.steps.filter((item) => item.advance.kind === "route-change")) {
    assert.equal(step.target, `nav.${step.advance.expectedRouteId}`);
    assert.equal(step.interaction, "target-only");
  }
  for (const step of manualTour.steps.filter((item) => !item.id.startsWith("navigate-"))) {
    assert.equal(step.interaction, "blocked");
  }
  assert.equal(manualTour.steps.at(-1).id, "recovery-mods");
  assert.deepEqual(manualTour.steps.at(-1).advance, { kind: "terminal" });

  const automaticTour = buildOnboardingTour("dashboard", { includeWelcome: true });
  assert.equal(automaticTour.steps[0].id, "welcome");
  assert.equal(automaticTour.steps[1].id, "page-dashboard");
  assert.equal(automaticTour.steps.length, 40);
  assert.equal(
    automaticTour.steps.find((step) => step.id === "recovery-actions")?.fallbackTarget,
    "recovery.actions",
  );
});

test("tour anchors are additive and preserve the existing dashboard status rail", () => {
  const classicSidebar = readProjectFile(
    "src/app/shell/layouts/classic-sidebar/ClassicSidebar.tsx",
  );
  const floatingSidebar = readProjectFile(
    "src/app/shell/layouts/floating-sidebar/FloatingSidebar.tsx",
  );
  const hero = readProjectFile("src/features/dashboard/DashboardHeroCard.tsx");
  const statusPanel = readProjectFile("src/features/dashboard/SetupStatusPanel.tsx");
  const routerOutlet = readProjectFile("src/app/routing/RouterOutlet.tsx");
  const gameDirectoryActions = readProjectFile("src/features/game-setup/GameDirectoryActions.tsx");
  const gamePrerequisites = readProjectFile("src/features/game-setup/GamePrerequisitePanel.tsx");
  const modToolbar = readProjectFile("src/features/mods/LibraryToolbar.tsx");
  const modActions = readProjectFile("src/features/mods/CompactActionPanel.tsx");
  const modLibrary = readProjectFile("src/features/mods/ModLibraryPage.tsx");
  const profileList = readProjectFile("src/features/profiles/ProfileListPanel.tsx");
  const profilePage = readProjectFile("src/features/profiles/ProfilePage.tsx");
  const backupPolicy = readProjectFile("src/features/profiles/BackupPolicyPanel.tsx");
  const saveDirectories = readProjectFile("src/features/profiles/SaveDirectoryPanel.tsx");
  const recovery = readProjectFile("src/features/install-recovery/RecoveryCenterPage.tsx");
  const categories = readProjectFile("src/features/categories/CategoryPage.tsx");
  const backups = readProjectFile("src/features/backups/BackupCenterPage.tsx");
  const diagnostics = readProjectFile("src/features/diagnostics/DiagnosticsPage.tsx");
  const settings = readProjectFile("src/features/settings/SettingsPage.tsx");
  const backgroundProtection = readProjectFile("src/features/settings/BackgroundProtectionPanel.tsx");

  assert.match(classicSidebar, /data-tour-id="app\.navigation"/);
  assert.match(floatingSidebar, /data-tour-id="app\.navigation"/);
  assert.match(classicSidebar, /data-tour-id=\{`nav\.\$\{item\.id\}`\}/);
  assert.match(floatingSidebar, /data-tour-id=\{`nav\.\$\{item\.id\}`\}/);
  assert.match(routerOutlet, /data-tour-id=\{`page\.\$\{layer\.route\.id\}`\}/);
  assert.match(hero, /data-tour-id="dashboard\.game-setup"/);
  assert.match(statusPanel, /data-tour-id="dashboard\.setup-status"/);
  assert.match(gameDirectoryActions, /data-tour-id="dashboard\.directory-actions"/);
  assert.match(gamePrerequisites, /data-tour-id=\{tourId\}/);
  assert.match(hero, /tourId="dashboard\.prerequisites"/);
  assert.match(modToolbar, /data-tour-id="mods\.toolbar"/);
  assert.match(modActions, /data-tour-id="mods\.actions"/);
  assert.match(modActions, /tourId="mods\.import-action"/);
  assert.match(modLibrary, /data-tour-id="mods\.library"/);
  assert.match(profileList, /data-tour-id="profiles\.list"/);
  assert.match(profilePage, /data-tour-id="profiles\.settings"/);
  assert.match(profilePage, /data-tour-id="profiles\.manual-backup"/);
  assert.match(profilePage, /data-tour-id="profiles\.auto-backup"/);
  assert.match(profilePage, /data-tour-id="profiles\.backup-history"/);
  assert.match(backupPolicy, /data-tour-id="profiles\.backup-policy"/);
  assert.match(saveDirectories, /data-tour-id="profiles\.save-directories"/);
  assert.match(recovery, /data-tour-id="recovery\.overview"/);
  assert.match(recovery, /data-tour-id="recovery\.manual-actions"/);
  assert.match(recovery, /data-tour-id="recovery\.mods"/);
  assert.match(categories, /data-tour-id="categories\.create"/);
  assert.match(categories, /data-tour-id="categories\.manage"/);
  assert.match(backups, /data-tour-id="backups\.filters"/);
  assert.match(backups, /data-tour-id="backups\.profiles"/);
  assert.match(backups, /data-tour-id="backups\.history"/);
  assert.match(diagnostics, /data-tour-id="diagnostics\.actions"/);
  assert.match(diagnostics, /data-tour-id="diagnostics\.health"/);
  assert.match(diagnostics, /data-tour-id="diagnostics\.logs"/);
  assert.match(settings, /tourId="settings\.appearance"/);
  assert.match(settings, /tourId="settings\.window-behavior"/);
  assert.match(settings, /tourId="settings\.prerequisites"/);
  assert.match(settings, /tourId="settings\.save-backup"/);
  assert.match(backgroundProtection, /data-tour-id="settings\.background-protection"/);
  assert.match(statusPanel, />下一步</);
  assert.match(statusPanel, />设置摘要</);
  assert.doesNotMatch(statusPanel, /FirstRunChecklist|first-run-checklist/);
});

test("tour positioning and stacking contracts avoid WebView and safety-overlay regressions", () => {
  const overlaySource = readProjectFile("src/shared/onboarding/TourOverlay.tsx");
  const targetSource = readProjectFile("src/shared/onboarding/useTourTarget.ts");
  const tourCss = readProjectFile("src/shared/onboarding/onboarding.css");
  const tokensCss = readProjectFile("src/shared/styles/tokens.css");
  const closeCss = readProjectFile("src/app/window-lifecycle/WindowCloseDialog.css");
  const forwardStepKeyframes = tourCss.match(
    /@keyframes tour-panel-step-forward\s*\{([\s\S]*?)\n\}/,
  )?.[1] ?? "";
  const tourZIndex = Number(tokensCss.match(/--z-tour:\s*(\d+)/)?.[1]);
  const closeZIndex = Number(closeCss.match(/\.window-close-overlay\s*\{[\s\S]*?z-index:\s*(\d+)/)?.[1]);

  assert.doesNotMatch(targetSource, /attributeFilter:[\s\S]*?"style"/);
  assert.match(targetSource, /resolvePreferredTourTarget\(primaryAnchor, fallbackAnchor\)/);
  assert.match(targetSource, /state\.requestKey === requestKey/);
  assert.match(targetSource, /interactionRect/);
  assert.match(targetSource, /TOUR_TARGET_WAIT_MS = 1_800/);
  assert.match(targetSource, /TOUR_TARGET_ANIMATION_POLL_MS = 1_200/);
  assert.match(targetSource, /window\.requestAnimationFrame\(pollAnimatedTarget\)/);
  assert.match(targetSource, /timedOut: true/);
  assert.match(tourCss, /\.tour-layer__blocker\.is-top/);
  assert.match(tourCss, /\.tour-layer__blocker\.is-bottom/);
  assert.match(tourCss, /\.tour-layer\s*\{[\s\S]*?pointer-events:\s*none;/);
  assert.match(overlaySource, /previousStepIndexRef/);
  assert.match(overlaySource, /className=\{positionerClassName\}/);
  assert.match(overlaySource, /useTourPanelRelocation\(positionerRef, panelRef/);
  assert.match(overlaySource, /const animation = panel\.animate\(/);
  assert.match(overlaySource, /TOUR_PANEL_RELOCATION_MS/);
  assert.match(overlaySource, /当前页面没有可高亮的对应区域/);
  assert.match(overlaySource, />重新定位</);
  assert.match(overlaySource, /"跳过此项"/);
  assert.match(overlaySource, /className=\{`tour-panel__stage is-\$\{stepDirection\}`\}/);
  assert.match(forwardStepKeyframes, /transform:/);
  assert.match(tourCss, /\.tour-panel-positioner\.is-welcome/);
  assert.match(tourCss, /\.tour-spotlight__cutout\s*\{/);
  assert.match(tourCss, /x 440ms cubic-bezier/);
  assert.match(tourCss, /top 440ms cubic-bezier/);
  assert.match(tourCss, /\.tour-layer\.is-closing \.tour-panel__stage/);
  assert.match(tourCss, /@keyframes tour-layer-enter/);
  assert.match(tourCss, /@keyframes tour-panel-enter/);
  assert.match(tourCss, /@keyframes tour-panel-step-backward/);
  assert.match(tourCss, /@keyframes tour-spotlight-enter/);
  assert.doesNotMatch(tourCss, /backdrop-filter/);
  assert.doesNotMatch(tourCss, /will-change/);
  assert.equal(tourZIndex, 190);
  assert.equal(closeZIndex, 200);
  assert.ok(tourZIndex < closeZIndex);
});
