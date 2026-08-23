import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { forbiddenDiscoveryFields } from "./testSupport/forbiddenDiscoveryFields.mjs";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("profiles route is registered and enabled from the shared navigation definition", () => {
  assert.equal(existsSync("src/features/profiles/ProfilePage.tsx"), true);
  assert.equal(existsSync("src/features/profiles/ProfilePage.css"), true);
  assert.equal(existsSync("src/features/profiles/ProfileSaveManager.css"), true);

  const routeTypes = readSource("src/app/routing/routeTypes.ts");
  const routeRegistry = readSource("src/app/routing/routeRegistry.tsx");
  const navItems = readSource("src/app/shell/navigation/navItems.ts");
  const main = readSource("src/main.tsx");

  assert.match(routeTypes, /"profiles"/);
  assert.match(routeRegistry, /import\s+\{\s*ProfilePage\s*\}\s+from\s+"..\/..\/features\/profiles\/ProfilePage"/);
  assert.match(routeRegistry, /id:\s*"profiles"[\s\S]*?path:\s*"\/profiles"[\s\S]*?element:\s*ProfilePage/);
  assert.match(navItems, /\{\s*id:\s*"profiles"[\s\S]*?route:\s*"\/profiles"[\s\S]*?\}/);
  assert.doesNotMatch(navItems.match(/\{\s*id:\s*"profiles"[\s\S]*?\}/)?.[0] ?? "", /disabledReason/);
  assert.match(main, /features\/profiles\/ProfilePage\.css/);
  assert.match(main, /features\/profiles\/ProfileSaveManager\.css/);
});

test("app shell provides and displays the active profile", () => {
  const app = readSource("src/App.tsx");
  const header = readSource("src/app/frame/AppHeader.tsx");

  assert.match(app, /ActiveProfileProvider/);
  assert.match(app, /<ActiveProfileProvider>[\s\S]*?<AppShell>/);
  assert.match(header, /useActiveProfile/);
  assert.match(header, /activeProfile\.status\s*===\s*"ready"/);
  assert.match(header, /activeProfile\.profile\.name/);
  assert.doesNotMatch(header, />待初始化</);
});

test("profile page exposes save settings workspace panels without shell coupling", () => {
  const source = readSource("src/features/profiles/ProfilePage.tsx");
  const css = readSource("src/features/profiles/ProfilePage.css");
  const saveManagerCss = readSource("src/features/profiles/ProfileSaveManager.css");

  assert.match(source, /className="profile-page__header"/);
  assert.match(source, /ProfileListPanel/);
  assert.match(source, /SaveDirectoryPanel/);
  assert.match(source, /BackupPolicyPanel/);
  assert.match(source, /listProfiles\(\)/);
  assert.match(source, /getProfileSaveSettings/);
  assert.match(source, /settingsRefreshToken/);
  assert.match(source, /setSettingsRefreshToken\(\(current\) => current \+ 1\)/);
  assert.match(source, /getProfileSaveSettings\(\{\s*gameId:\s*CURRENT_GAME_ID,\s*profileId:\s*selectedProfileId\s*\}\)/);
  assert.match(source, /setProfileSaveSettings/);
  assert.match(source, /setActiveProfile/);
  assert.match(source, /refreshActiveProfile/);
  assert.doesNotMatch(source, /useSidebarMode|sidebarMode/);
  assert.doesNotMatch(source, /manifestPath|backupRoot|backupRef|targetPath|sandbox|cache/i);
  assert.match(css, /\.route-transition__layer\[data-route-id="profiles"\]/);
  assert.match(css, /\.profile-workspace/);
  assert.match(css, /\.profile-settings-panel/);
  assert.match(saveManagerCss, /\.profile-directory-summary/);
  assert.match(saveManagerCss, /\.profile-directory-row/);
  assert.doesNotMatch(css, /profile-page__summary-grid|profile-main-card|profile-row/);
  assert.match(css, /@media\s*\(max-width:\s*860px\)/);
});

test("存档路径行在窄容器下重排而不是把文字压到逐字换行", () => {
  const saveManagerCss = readSource("src/features/profiles/ProfileSaveManager.css");
  const pageCss = readSource("src/features/profiles/ProfilePage.css");

  // 标签所在列是 minmax(0, 1fr)，右侧操作组是 min-width: max-content 不肯收缩，
  // 因此标签能被压到接近零宽。中文没有词边界，不禁止换行就会逐字竖排。
  const labelRule = /\.profile-directory-row__copy span \{([\s\S]*?)\}/.exec(saveManagerCss);
  assert.ok(labelRule, "缺少存档路径行的标签样式");
  assert.match(labelRule[1], /white-space:\s*nowrap;/);
  assert.match(labelRule[1], /text-overflow:\s*ellipsis;/);

  // 面板嵌在两层网格里，实际可用宽度由外层分配决定，视口断点判断不出它何时变窄。
  const consoleRule = /\.profile-directory-console \{([\s\S]*?)\}/.exec(saveManagerCss);
  assert.ok(consoleRule, "缺少存档路径面板样式");
  assert.match(consoleRule[1], /container-type:\s*inline-size;/);

  // 窄容器下操作组换到第二行，让文字与按钮各自拿到完整宽度。
  assert.match(
    saveManagerCss,
    /@container \(max-width: 380px\)[\s\S]*?\.profile-directory-row \{[\s\S]*?grid-template-areas:/,
    "缺少窄容器下的存档路径行两段式重排",
  );

  // 配置卡包列宽会一路传导到存档路径面板的可用宽度，不得回涨。
  const workspaceRule = /\.profile-workspace \{([\s\S]*?)\}/.exec(pageCss);
  assert.ok(workspaceRule, "缺少 profile-workspace 布局");
  const [, minWidth, maxWidth] = /minmax\((\d+)px,\s*(\d+)px\)/.exec(workspaceRule[1]);
  assert.ok(
    Number(minWidth) <= 240 && Number(maxWidth) <= 280,
    `配置卡包列 ${minWidth}-${maxWidth}px 过宽，会挤压存档路径面板`,
  );
});

test("activating a profile also selects it before refreshing its settings", () => {
  const source = readSource("src/features/profiles/ProfilePage.tsx");
  const activationHandler =
    source.match(/const handleActivateProfile = async \(profileId: string\) => \{[\s\S]*?^ {2}\};/m)?.[0] ?? "";

  assert.match(
    activationHandler,
    /await setActiveProfile\(profileId\);[\s\S]*?setSelectedProfileId\(profileId\);[\s\S]*?refreshProfiles\(\);/,
  );
});

test("profile create and edit forms use a floating dialog instead of inline list replacement", () => {
  const listSource = readSource("src/features/profiles/ProfileListPanel.tsx");
  const css = readSource("src/features/profiles/ProfilePage.css");

  assert.match(listSource, /profile-list-panel__floating-root/);
  assert.match(listSource, /createPortal\(/);
  assert.match(listSource, /document\.body/);
  assert.match(listSource, /className="profile-floating-backdrop"/);
  assert.match(listSource, /className="profile-floating-form"/);
  assert.match(listSource, /role="dialog"/);
  assert.match(listSource, /aria-modal="true"/);
  assert.match(listSource, /aria-label=\{showCreateForm \? copy\.createTitle : copy\.editTitle\}/);
  const listCopySource = readSource("src/features/profiles/profileListCopy.ts");
  assert.match(listCopySource, /createTitle: "新建配置档"/);
  assert.match(listCopySource, /editTitle: "编辑配置档"/);
  assert.match(listSource, /profile-floating-form__header/);
  assert.match(listSource, /<textarea[\s\S]*?rows=\{4\}/);
  assert.match(listSource, /document\.addEventListener\("mousedown", handlePointerDown\)/);
  assert.match(listSource, /event\.key === "Escape"/);
  assert.match(listSource, /copy\.cannotDeleteActive/);
  assert.match(listCopySource, /cannotDeleteActive: "当前配置档不能删除"/);
  assert.doesNotMatch(listSource, /editingId === profile\.id\s*\?/);
  assert.doesNotMatch(listSource, /className="profile-list-item is-editing"/);
  assert.match(css, /\.profile-list-panel__floating-root\s*\{[\s\S]*?position:\s*relative/);
  assert.match(css, /\.profile-floating-form\s*\{[\s\S]*?position:\s*fixed/);
  assert.match(css, /\.profile-floating-form\s*\{[\s\S]*?top:\s*50%/);
  assert.match(css, /\.profile-floating-form\s*\{[\s\S]*?left:\s*50%/);
  assert.match(css, /\.profile-floating-form\s*\{[\s\S]*?width:\s*min\(560px/);
  assert.match(css, /\.profile-floating-form\s*\{[\s\S]*?translate:\s*-50%\s*-50%/);
  assert.match(css, /\.profile-floating-form\s*\{[\s\S]*?box-shadow:\s*var\(--shadow-panel\)/);
  assert.match(css, /\.profile-floating-backdrop\s*\{[\s\S]*?backdrop-filter:\s*blur\(5px\)/);
  assert.match(css, /\.profile-field textarea\s*\{[\s\S]*?min-height:\s*118px/);
  assert.match(css, /\.profile-floating-form \.profile-inline-form\s*\{[\s\S]*?background:\s*transparent/);
});

test("profile primary actions preserve readable text across hover and active states", () => {
  const css = readSource("src/features/profiles/ProfilePage.css");
  const hoverRule = css.match(/\.profile-action-button\.is-primary:not\(:disabled\):hover\s*\{[^}]*\}/)?.[0] ?? "";
  const activeRule = css.match(/\.profile-action-button\.is-primary:not\(:disabled\):active\s*\{[^}]*\}/)?.[0] ?? "";

  assert.match(hoverRule, /color:\s*var\(--color-primary-action-text\)/);
  assert.match(hoverRule, /border-color:\s*var\(--color-primary-action-border\)/);
  assert.match(activeRule, /color:\s*var\(--color-primary-action-text\)/);
});

test("save directory picker catches dialog and validation failures consistently", () => {
  const source = readSource("src/features/profiles/SaveDirectoryPanel.tsx");
  const chooseDirectoryBlock = source.match(/const chooseDirectory[\s\S]*?^ {2}};/m)?.[0] ?? "";

  assert.match(chooseDirectoryBlock, /setBusyKind\(kind\);[\s\S]*?try\s*\{/);
  assert.match(chooseDirectoryBlock, /const selected = await open\(\{ directory: true, multiple: false \}\);/);
  assert.match(chooseDirectoryBlock, /catch \(err\) \{[\s\S]*setError\(getPanelErrorMessage\(err, copy\.panel\.errorFallback\)\)/);
  assert.match(chooseDirectoryBlock, /finally \{[\s\S]*setBusyKind\(null\)/);
});

test("auto backup controls stay interactive when persistence is unavailable", () => {
  const pageSource = readSource("src/features/profiles/ProfilePage.tsx");
  const pickerSource = readSource("src/features/profiles/BackupSchedulePicker.tsx");
  const viewModelSource = readSource("src/features/profiles/profileViewModel.ts");
  const css = readSource("src/features/profiles/ProfilePage.css");
  const backupPanelBlock = pageSource.match(/<BackupPolicyPanel[\s\S]*?\/>/)?.[0] ?? "";

  assert.match(pageSource, /draftSettings/);
  assert.match(pageSource, /setDraftSettings\(createPreviewSaveSettings\(selectedProfileId\)\)/);
  assert.match(pageSource, /const visibleSettings = draftSettings/);
  assert.doesNotMatch(backupPanelBlock, /disabled=\{!settingsEditable\}/);
  assert.match(pageSource, /schedule:\s*draftSettings\.schedule/);
  assert.match(pickerSource, /picker\.weekdayAbbr\[day\]/);
  const policyCopySource = readSource("src/features/profiles/backupPolicyCopy.ts");
  assert.match(policyCopySource, /1: "星期一"/);
  assert.match(policyCopySource, /0: "星期日"/);
  assert.match(pickerSource, /Array\.from\(\{\s*length:\s*60\s*\}/);
  assert.match(pickerSource, /function wrapIndex/);
  assert.match(pickerSource, /\(\(index % length\) \+ length\) % length/);
  assert.match(pickerSource, /scrollPickerDisplayOffsets\s*=\s*\[-2,\s*-1,\s*0,\s*1,\s*2\]/);
  assert.match(pickerSource, /selectedIndex \+ offset/);
  assert.match(pickerSource, /getWheelItemStyle/);
  assert.match(pickerSource, /rotateX/);
  assert.match(pickerSource, /addEventListener\("wheel",\s*handleWheel,\s*\{\s*passive:\s*false\s*\}\)/);
  assert.doesNotMatch(pickerSource, /onWheel=\{handleWheel\}/);
  assert.match(pickerSource, /onPointerMove=\{handlePointerMove\}/);
  assert.match(pickerSource, /weekdayDisplayOrder/);
  assert.match(viewModelSource, /weekdays:\s*\[1\]/);
  assert.match(viewModelSource, /weekdayDisplayOrder/);
  assert.match(viewModelSource, /\[0,\s*6\]/);
  assert.match(css, /perspective:\s*420px/);
  assert.match(css, /transform-style:\s*preserve-3d/);
});

test("profile save UI follows the redesigned structure without inline styling", () => {
  const pageSource = readSource("src/features/profiles/ProfilePage.tsx");
  const listSource = readSource("src/features/profiles/ProfileListPanel.tsx");
  const directorySource = readSource("src/features/profiles/SaveDirectoryPanel.tsx");
  const pickerSource = readSource("src/features/profiles/BackupSchedulePicker.tsx");
  const css = readSource("src/features/profiles/ProfilePage.css");
  const saveManagerCss = readSource("src/features/profiles/ProfileSaveManager.css");
  const detailConsoleStart = pageSource.indexOf("profile-detail-console");
  const readyDeckStart = pageSource.indexOf('{settingsState.status === "ready" ? (', detailConsoleStart);
  const detailConsoleBlock = pageSource.slice(detailConsoleStart, readyDeckStart);

  assert.match(listSource, /slot-meta/);
  assert.match(listSource, /slot-num/);
  assert.match(listSource, /slot-badge/);
  assert.match(listSource, /slot-title/);
  assert.match(listSource, /slot-desc/);
  assert.doesNotMatch(listSource, /style=\{\{/);

  assert.match(directorySource, /profile-directory-summary/);
  assert.match(directorySource, /profile-directory-row__path/);
  assert.doesNotMatch(pageSource, /directory-flow-connector|directory-flow-badge|directory-flow-line/);
  assert.match(pageSource, /ProfileHeaderSaveAction/);
  assert.match(pageSource, /className="profile-page__actions/);
  assert.match(pageSource, /className=\{`profile-header-save-action/);
  assert.match(pageSource, /onSave=\{\(\) => void saveSettings\(\)\}/);
  assert.doesNotMatch(pageSource, /function ProfileOverview|function ProfileMetric/);
  assert.doesNotMatch(pageSource, /profile-overview|profile-toolbar-save-box|profile-metric/);
  assert.match(pageSource, /profile-save-manager-deck/);
  assert.match(pageSource, /ActiveSavePanel/);
  assert.match(pageSource, /BackupHistoryPanel/);
  assert.match(
    pageSource,
    /<div className="profile-save-manager-deck save-manager-deck">[\s\S]*?<div className="profile-save-strategy-stack[\s\S]*?<div className="profile-directory-zone">[\s\S]*?<SaveDirectoryPanel[\s\S]*?<BackupHistoryPanel/,
  );
  assert.doesNotMatch(detailConsoleBlock, /SaveDirectoryPanel/);
  assert.doesNotMatch(pageSource, /存档沙盒隔离|安装 Mod 前备份|自动归档计划/);
  assert.doesNotMatch(pageSource, /profile-policy-flags|PolicyFlag/);
  assert.match(pickerSource, /schedule-chip/);
  assert.match(pickerSource, /scroll-picker-arrow/);

  assert.match(css, /:root\[data-color-scheme="light"\]\s+\.profile-page/);
  assert.match(css, /:root\[data-color-scheme="dark"\]\s+\.profile-page/);
  assert.match(css, /\.profile-header-save-action/);
  assert.doesNotMatch(css, /profile-overview|profile-toolbar-save-box|profile-metric/);
  assert.match(saveManagerCss, /\.profile-save-manager-deck/);
  assert.match(saveManagerCss, /\.active-save-banner/);
  assert.doesNotMatch(saveManagerCss, /profile-overview|profile-toolbar-save-box|profile-metric/);
  assert.doesNotMatch(saveManagerCss, /profile-policy-flags|profile-policy-flag|profile-policy-switch/);
  assert.match(pageSource, /className="profile-backup-list"/);
  assert.match(pageSource, /className="profile-backup-item"/);
  assert.match(pageSource, /\{copy\.history\.restore\}/);
  const pageCopySource = readSource("src/features/profiles/profilePageCopy.ts");
  assert.match(pageCopySource, /restore: "恢复存档"/);
  assert.doesNotMatch(pageSource, /即将开放/);
  assert.match(saveManagerCss, /\.profile-backup-list/);
  assert.match(saveManagerCss, /\.profile-backup-item/);
  assert.match(saveManagerCss, /\.profile-backup-item__actions/);
  assert.doesNotMatch(pageSource, /profile-backup-table/);
  assert.doesNotMatch(saveManagerCss, /\.profile-backup-table|overflow-x:\s*auto|min-width:\s*520px/);
  assert.match(saveManagerCss, /\.profile-save-manager-deck\.save-manager-deck\s*\{[\s\S]*?overflow:\s*visible/);
  assert.match(saveManagerCss, /\.profile-save-manager-deck\.save-manager-deck\s*\{[\s\S]*?grid-template-areas:\s*"strategy directories"\s*"strategy history"/);
  assert.match(saveManagerCss, /\.profile-directory-zone\s*\{[\s\S]*?grid-area:\s*directories/);
  assert.match(saveManagerCss, /\.profile-save-strategy-stack\.strategy-card\s*\{[\s\S]*?grid-area:\s*strategy/);
  assert.match(saveManagerCss, /\.profile-history-card\.history-card\s*\{[\s\S]*?grid-area:\s*history/);
  assert.match(saveManagerCss, /\.profile-directory-summary\s*\{[\s\S]*?padding:\s*12px/);
  assert.match(saveManagerCss, /\.profile-directory-row\s*\{[\s\S]*?grid-template-columns:\s*auto minmax\(0,\s*1fr\) auto/);
  assert.doesNotMatch(saveManagerCss, /\.profile-directory-grid\.paths-setup-grid/);
  assert.doesNotMatch(saveManagerCss, /\.profile-directory-card\.path-card/);
  assert.match(saveManagerCss, /\.profile-save-strategy-stack\.strategy-card\s*\{[\s\S]*?z-index:\s*20/);
  assert.match(saveManagerCss, /\.profile-save-strategy-stack\.strategy-card \.backup-schedule-popover\s*\{[\s\S]*?z-index:\s*200/);
  assert.match(saveManagerCss, /\.profile-save-strategy-stack\.strategy-card \.backup-schedule-popover\s*\{[\s\S]*?bottom:\s*0/);
  assert.match(saveManagerCss, /\.profile-history-card\.history-card\s*\{[\s\S]*?z-index:\s*1/);
  assert.doesNotMatch(css, /directory-flow-connector|directory-flow-badge|directory-flow-line/);
  assert.doesNotMatch(css, /\.profile-save-bar\s*\{[^}]*display:\s*none/);
});

test("profile page wires manual save backup execution, progress, and history refresh", () => {
  const pageSource = readSource("src/features/profiles/ProfilePage.tsx");
  const taskStateSource = readSource("src/features/profiles/profileSaveBackupTaskState.ts");
  const saveManagerCss = readSource("src/features/profiles/ProfileSaveManager.css");
  const startBackupCall = pageSource.match(/startProfileSaveBackup\(\{[\s\S]*?\}\);/)?.[0] ?? "";

  assert.match(pageSource, /startProfileSaveBackup/);
  assert.match(pageSource, /listProfileSaveBackups/);
  assert.match(pageSource, /listen<TaskProgressEventDto>\(TASK_PROGRESS_EVENT_NAME/);
  assert.match(pageSource, /nextProfileSaveBackupTaskStateFromProgress/);
  assert.match(pageSource, /shouldRefreshProfileSaveBackupHistory/);
  assert.match(pageSource, /setBackupHistoryRefreshToken\(\(current\) => current \+ 1\)/);
  assert.match(pageSource, /pendingBackupCompletionToastRef/);
  assert.match(pageSource, /saveBackupTaskProfileIdsRef/);
  assert.match(pageSource, /attachStartedSaveBackupTask\(task, selectedProfileId\)/);
  assert.match(pageSource, /taskProfileId = saveBackupTaskProfileIdsRef\.current\.get\(saveBackupTaskState\.taskId\)/);
  assert.match(pageSource, /profileId:\s*taskProfileId/);
  assert.match(pageSource, /publishPendingBackupCompletionToast/);
  assert.match(pageSource, /eventKey:\s*`profile\.save-backup\.completed\.\$\{pending\.taskId\}`/);
  assert.match(pageSource, /taskId:\s*pending\.taskId/);
  assert.doesNotMatch(pageSource, /ProfileManualBackupFloatingNotice|manualBackupNotice/);
  assert.match(pageSource, /onClick=\{\(\) => void startManualSaveBackup\(\)\}/);
  assert.match(pageSource, /disabled=\{!canStartManualSaveBackup\}/);
  assert.doesNotMatch(pageSource, /profile-create-backup-button" disabled/);
  assert.doesNotMatch(startBackupCall, /saveDirectory|backupDirectory|path|manifest|backupRef|hash/i);

  assert.match(taskStateSource, /save_backup\.queued/);
  assert.match(taskStateSource, /save_backup\.retention_pruning/);
  assert.match(taskStateSource, /event\.kind !== "save_backup"/);
  assert.match(taskStateSource, /current\.taskId !== event\.taskId/);

  assert.match(saveManagerCss, /\.profile-manual-backup-card/);
  assert.match(saveManagerCss, /\.profile-manual-backup-status/);
  assert.match(saveManagerCss, /\.profile-create-backup-button\.profile-action-button\.is-primary\s*\{[\s\S]*?color:\s*#fff/);
  assert.match(saveManagerCss, /\.profile-backup-list/);
  assert.match(saveManagerCss, /\.profile-backup-restore-button/);
});

test("backup history keeps the restore entry visible without horizontal scrolling", () => {
  const pageSource = readSource("src/features/profiles/ProfilePage.tsx");
  const saveManagerCss = readSource("src/features/profiles/ProfileSaveManager.css");

  assert.match(pageSource, /role="list" aria-label=\{copy\.history\.listAria\}/);
  assert.match(
    readSource("src/features/profiles/profilePageCopy.ts"),
    /listAria: "备份历史"/,
  );
  assert.match(pageSource, /role="listitem"/);
  assert.match(pageSource, /<ArchiveRestore size=\{15\}/);
  assert.match(pageSource, /aria-label=\{copy\.history\.restoreAria\(row\.name\)\}/);
  assert.match(saveManagerCss, /\.profile-backup-list\s*\{[\s\S]*?container-type:\s*inline-size/);
  assert.match(saveManagerCss, /\.profile-backup-item\s*\{[\s\S]*?grid-template-columns:\s*minmax\(0,\s*1fr\)/);
  assert.match(saveManagerCss, /@container \(min-width:\s*380px\)[\s\S]*?\.profile-backup-item\s*\{[\s\S]*?grid-template-columns:\s*minmax\(0,\s*1fr\) auto/);
  assert.match(saveManagerCss, /@container \(min-width:\s*680px\)[\s\S]*?\.profile-backup-item\s*\{[\s\S]*?grid-template-columns:\s*minmax\(0,\s*1fr\) minmax\(172px,\s*auto\) auto/);
  assert.doesNotMatch(saveManagerCss, /profile-backup[^}]*overflow-x:\s*auto/);
});

test("profile page wires client runtime auto backup checks honestly", () => {
  const pageSource = readSource("src/features/profiles/ProfilePage.tsx");
  const apiSource = readSource("src/features/profiles/profileSaveBackupApi.ts");
  const typesSource = readSource("src/features/profiles/profileSaveBackupTypes.ts");
  const saveManagerCss = readSource("src/features/profiles/ProfileSaveManager.css");
  const autoCheckCall = pageSource.match(/checkProfileAutoSaveBackup\(\{[\s\S]*?\}\);/)?.[0] ?? "";
  const autoCheckEffect = pageSource.match(/useEffect\(\(\) => \{[\s\S]*?checkProfileAutoSaveBackup[\s\S]*?\}, \[[^\]]+\]\);/)?.[0] ?? "";
  const autoCheckDependencyMatches = [...autoCheckEffect.matchAll(/\}, \[([^\]]+)\]\);/g)];
  const autoCheckDependencies = autoCheckDependencyMatches.at(-1)?.[1] ?? "";
  const autoCheckDependencyNames = autoCheckDependencies
    .split(",")
    .map((dependency) => dependency.trim())
    .filter(Boolean);

  assert.match(apiSource, /checkProfileAutoSaveBackup/);
  assert.match(apiSource, /check_auto_save_backup/);
  assert.match(typesSource, /ProfileAutoSaveBackupCheckDto/);
  assert.match(pageSource, /checkProfileAutoSaveBackup/);
  assert.match(pageSource, /getBackgroundProtectionCopy\(settings\.schedule\.cadence, backgroundState, copy\.background, locale\)/);
  const bgCopySource = readSource("src/features/profiles/profilePageCopy.ts");
  assert.match(bgCopySource, /unsupportedHint: "自动备份仅在客户端运行时执行"/);
  assert.match(pageSource, /getSaveBackupBackgroundStatus/);
  assert.match(bgCopySource, /trayOnlyLabel: "仅客户端运行期保护"/);
  assert.match(bgCopySource, /notEnabledLabel: "未启用后台保护"/);
  assert.match(apiSource, /get_save_backup_background_status/);
  assert.match(typesSource, /SaveBackupBackgroundStatusDto/);
  assert.doesNotMatch(typesSource, /leaseOwner|leaseExpiresAt|workerInstanceId/);
  assert.match(pageSource, /startedTask/);
  assert.match(typesSource, /pendingReason: SaveBackupPendingReason \| null/);
  assert.match(bgCopySource, /deferredGameRunning: "游戏运行中，自动备份已延后"/);
  assert.match(bgCopySource, /deferredGameUnknown: "暂时无法确认游戏状态，备份已延后"/);
  assert.match(pageSource, /AutoSaveBackupRuntimePanel/);
  assert.match(pageSource, /getAutoBackupCheckBlockedReason\(saveBackupTaskState, pageCopy\.blockedReasons\)/);
  assert.match(pageSource, /disabledReason=\{autoBackupCheckBlockedReason\}/);
  assert.match(pageSource, /const disabled = checking \|\| disabledReason !== null/);
  assert.match(pageSource, /setSaveBackupTaskState/);
  assert.match(autoCheckEffect, /settingsState\.status/);
  assert.ok(autoCheckDependencyNames.includes("settingsState.status"));
  assert.equal(autoCheckDependencyNames.includes("settingsState"), false);
  assert.doesNotMatch(autoCheckCall, /saveDirectory|backupDirectory|path|manifest|backupRef|hash/i);
  assert.match(saveManagerCss, /\.profile-auto-backup-card/);
  assert.match(saveManagerCss, /\.profile-auto-backup-protection/);
});

test("profile background status supports starting without an enable toggle", () => {
  const typesSource = readSource("src/features/profiles/profileSaveBackupTypes.ts");
  const pageSource = readSource("src/features/profiles/ProfilePage.tsx");
  const pageCss = readSource("src/features/profiles/ProfilePage.css");
  const badgeHelper =
    pageSource.match(/function getBackgroundProtectionBadge[\s\S]*?function getBackgroundProtectionCopy/)?.[0] ?? "";
  const settingsNavigationHelper =
    pageSource.match(/function shouldOfferBackgroundSettingsNavigation[\s\S]*?^}/m)?.[0] ?? "";

  assert.match(typesSource, /"starting"/);
  assert.match(pageSource, /case "starting"/);
  const bgPinSource = readSource("src/features/profiles/profilePageCopy.ts");
  assert.match(bgPinSource, /startingLabel: "正在验证后台保护"/);
  assert.match(bgPinSource, /badgeProtected: "退出后受保护"/);
  assert.match(bgPinSource, /badgeStarting: "等待后台验证"/);
  assert.match(bgPinSource, /badgeManual: "未启用自动备份"/);
  assert.match(pageSource, /navigate\("\/settings"\)/);
  assert.match(pageSource, /copy\.autoBackup\.goToSettings/);
  assert.match(bgPinSource, /goToSettings: "前往设置处理"/);
  assert.match(pageSource, /getBackgroundProtectionBadge/);
  assert.match(pageCss, /\.profile-background-settings-link/);
  assert.match(
    pageCss,
    /\.profile-auto-backup-card \.profile-auto-backup-protection__copy (?:strong|span)[\s\S]*?white-space:\s*normal/,
  );
  assert.match(badgeHelper, /cadence === "manual"[\s\S]*?copy\.badgeManual/);
  assert.match(badgeHelper, /status === "protected"[\s\S]*?copy\.badgeProtected/);
  assert.match(badgeHelper, /status === "starting"[\s\S]*?copy\.badgeStarting/);
  assert.match(badgeHelper, /client-only[\s\S]*?copy\.badgeClientOnly/);
  assert.match(bgPinSource, /badgeClientOnly: "仅客户端运行时"/);
  assert.match(settingsNavigationHelper, /registration_failed/);
  assert.match(settingsNavigationHelper, /worker_unhealthy/);
  assert.match(settingsNavigationHelper, /permission_required/);
  assert.doesNotMatch(settingsNavigationHelper, /protected|starting|not_enabled|unsupported_platform/);
  assert.doesNotMatch(pageSource, /enable_save_backup_background_protection/);
  assert.doesNotMatch(pageSource, /disable_save_backup_background_protection/);
  assert.doesNotMatch(pageSource, /status:\s*"protected"/);
});

test("plain browser preview renders the redesigned profiles console instead of the error shell", () => {
  const pageSource = readSource("src/features/profiles/ProfilePage.tsx");
  const directorySource = readSource("src/features/profiles/SaveDirectoryPanel.tsx");

  const previewDataSource = readSource("src/features/profiles/profilesPreviewData.ts");
  assert.match(previewDataSource, /PREVIEW_PROFILES/);
  assert.match(pageSource, /function isPlainBrowserRuntime/);
  assert.match(pageSource, /!"__TAURI_INTERNALS__" in window|!\("__TAURI_INTERNALS__" in window\)/);
  assert.match(pageSource, /createPreviewProfiles\(\)/);
  assert.match(pageSource, /data-preview-mode/);
  assert.match(pageSource, /setProfileState\(\{\s*status:\s*"ready",\s*profiles:\s*previewProfiles\s*\}\)/);
  assert.match(pageSource, /setSettingsState\(\{\s*status:\s*"ready",\s*settings\s*\}\)/);
  assert.match(directorySource, /previewMode\?:\s*boolean/);
  assert.match(directorySource, /if \(previewMode\)/);
  assert.match(directorySource, /createPreviewDirectorySelection\(kind\)/);
  assert.match(previewDataSource, /Steam\/userdata\/<steam-id>\/582010\/remote/);
});

test("profile save discovery uses shared toast feedback and candidate confirmation UI", () => {
  const app = readSource("src/App.tsx");
  const main = readSource("src/main.tsx");
  const page = readSource("src/features/profiles/ProfilePage.tsx");
  const panel = readSource("src/features/profiles/SaveDirectoryPanel.tsx");
  const provider = readSource("src/features/profiles/ProfileSaveDirectoryDiscoveryProvider.tsx");
  const candidates = readSource("src/features/profiles/ProfileSaveDirectoryCandidateList.tsx");
  const css = readSource("src/features/profiles/ProfileSaveDirectoryDiscovery.css");

  assert.match(app, /ProfileSaveDirectoryDiscoveryProvider/);
  assert.match(main, /ProfileSaveDirectoryDiscovery\.css/);
  assert.match(page, /ProfileSaveDirectoryCandidateList/);
  assert.match(panel, /copy\.panel\.autoDetect/);
  assert.match(
    readSource("src/features/profiles/saveDirectoryCopy.ts"),
    /autoDetect: "自动检测"/,
  );
  assert.match(provider, /useFeedback/);
  assert.match(provider, /pushToast\(\{/);
  assert.match(provider, /eventKey:\s*`profile\.save-directory\.\$\{notice\.profileId\}/);
  assert.match(provider, /label: copy\.noticeActions\.reviewCandidates/);
  assert.match(provider, /label: copy\.noticeActions\.retryDetection/);
  const dirCopySource = readSource("src/features/profiles/saveDirectoryCopy.ts");
  assert.match(dirCopySource, /reviewCandidates: "查看候选"/);
  assert.match(dirCopySource, /retryDetection: "重新检测"/);
  assert.match(candidates, /accountName/);
  assert.match(candidates, /avatarUrl/);
  assert.match(candidates, /recommended/);
  assert.doesNotMatch(css, /\.profile-save-directory-floating-notice/);
  assert.doesNotMatch(page + panel + provider + candidates, forbiddenDiscoveryFields);
});

test("profile save discovery guards stale async results and scopes busy state per profile", () => {
  const page = readSource("src/features/profiles/ProfilePage.tsx");
  const provider = readSource("src/features/profiles/ProfileSaveDirectoryDiscoveryProvider.tsx");
  const panel = readSource("src/features/profiles/SaveDirectoryPanel.tsx");

  assert.match(provider, /discoveryRequestSeqRef/);
  assert.match(provider, /discoveringTarget/);
  assert.match(provider, /isCurrentDiscoveryRequest/);
  assert.match(provider, /requestSeq/);
  assert.match(provider, /isTauri\(\)/);
  assert.match(provider, /discovery\.outcome === "scan_failed"/);
  assert.match(provider, /discovery\.outcome === "existing_invalid"/);

  const confirmCandidate = provider.slice(
    provider.indexOf("const confirmCandidate"),
    provider.indexOf("useEffect(() =>"),
  );
  assert.match(confirmCandidate, /const requestSeq = discoveryRequestSeqRef\.current \+ 1/);
  assert.match(confirmCandidate, /activeDiscoveryRequestRef\.current = requestSnapshot/);
  assert.match(
    confirmCandidate,
    /const discovery = await confirmProfileSaveDirectoryCandidate[\s\S]*?if \(!isCurrentDiscoveryRequest\(activeDiscoveryRequestRef\.current, requestSnapshot\)\) return;[\s\S]*?setLatestDiscovery\(discovery\)/,
  );
  assert.match(
    confirmCandidate,
    /catch \{[\s\S]*?if \(!isCurrentDiscoveryRequest\(activeDiscoveryRequestRef\.current, requestSnapshot\)\) return;[\s\S]*?setNotice/,
  );
  assert.match(
    confirmCandidate,
    /finally \{[\s\S]*?if \(isCurrentDiscoveryRequest\(activeDiscoveryRequestRef\.current, requestSnapshot\)\) \{[\s\S]*?activeDiscoveryRequestRef\.current = null;[\s\S]*?setDiscoveringTarget\(null\);[\s\S]*?setIsDiscovering\(false\);[\s\S]*?\}/,
  );
  assert.match(page, /autoDetecting=\{isDiscovering\s*&&\s*discoveringTarget\?\.profileId === selectedProfileId/);
  assert.match(panel, /profile-directory-row__button \$\{hasDiscoveryCandidates \? "is-primary" : ""\}/);
});
