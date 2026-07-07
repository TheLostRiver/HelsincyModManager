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
  assert.match(css, /\.profile-directory-grid/);
  assert.match(css, /\.profile-directory-card/);
  assert.doesNotMatch(css, /profile-page__summary-grid|profile-main-card|profile-row/);
  assert.doesNotMatch(css, /\.profile-directory-row/);
  assert.match(css, /@media\s*\(max-width:\s*860px\)/);
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
  assert.match(listSource, /aria-label=\{showCreateForm \? "新建配置档" : "编辑配置档"\}/);
  assert.match(listSource, /profile-floating-form__header/);
  assert.match(listSource, /<textarea[\s\S]*?rows=\{4\}/);
  assert.match(listSource, /document\.addEventListener\("mousedown", handlePointerDown\)/);
  assert.match(listSource, /event\.key === "Escape"/);
  assert.match(listSource, /当前配置档不能删除/);
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

test("save directory picker catches dialog and validation failures consistently", () => {
  const source = readSource("src/features/profiles/SaveDirectoryPanel.tsx");
  const chooseDirectoryBlock = source.match(/const chooseDirectory[\s\S]*?^ {2}};/m)?.[0] ?? "";

  assert.match(chooseDirectoryBlock, /setBusyKind\(kind\);[\s\S]*?try\s*\{/);
  assert.match(chooseDirectoryBlock, /const selected = await open\(\{ directory: true, multiple: false \}\);/);
  assert.match(chooseDirectoryBlock, /catch \(err\) \{[\s\S]*setError\(getPanelErrorMessage\(err\)\)/);
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
  assert.match(pickerSource, /label:\s*"星期一"/);
  assert.match(pickerSource, /label:\s*"星期日"/);
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
  assert.match(pickerSource, /weekdayOrder/);
  assert.match(viewModelSource, /weekdays:\s*\[1\]/);
  assert.match(viewModelSource, /\[0,\s*"星期日"\]/);
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

  assert.match(listSource, /slot-meta/);
  assert.match(listSource, /slot-num/);
  assert.match(listSource, /slot-badge/);
  assert.match(listSource, /slot-title/);
  assert.match(listSource, /slot-desc/);
  assert.doesNotMatch(listSource, /style=\{\{/);

  assert.match(directorySource, /profile-directory-grid/);
  assert.match(directorySource, /profile-directory-card__path/);
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
  assert.match(saveManagerCss, /\.profile-backup-table/);
  assert.match(saveManagerCss, /\.profile-save-manager-deck\.save-manager-deck\s*\{[\s\S]*?overflow:\s*visible/);
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
  assert.match(saveManagerCss, /\.profile-backup-table/);
});

test("profile page wires client runtime auto backup checks honestly", () => {
  const pageSource = readSource("src/features/profiles/ProfilePage.tsx");
  const apiSource = readSource("src/features/profiles/profileSaveBackupApi.ts");
  const typesSource = readSource("src/features/profiles/profileSaveBackupTypes.ts");
  const saveManagerCss = readSource("src/features/profiles/ProfileSaveManager.css");
  const autoCheckCall = pageSource.match(/checkProfileAutoSaveBackup\(\{[\s\S]*?\}\);/)?.[0] ?? "";

  assert.match(apiSource, /checkProfileAutoSaveBackup/);
  assert.match(apiSource, /check_auto_save_backup/);
  assert.match(typesSource, /ProfileAutoSaveBackupCheckDto/);
  assert.match(pageSource, /checkProfileAutoSaveBackup/);
  assert.match(pageSource, /仅在客户端运行时/);
  assert.match(pageSource, /退出主客户端后的后台保障尚未启用/);
  assert.match(pageSource, /startedTask/);
  assert.match(pageSource, /AutoSaveBackupRuntimePanel/);
  assert.match(pageSource, /getAutoBackupCheckBlockedReason\(saveBackupTaskState\)/);
  assert.match(pageSource, /disabledReason=\{autoBackupCheckBlockedReason\}/);
  assert.match(pageSource, /const disabled = checking \|\| disabledReason !== null/);
  assert.match(pageSource, /setSaveBackupTaskState/);
  assert.doesNotMatch(autoCheckCall, /saveDirectory|backupDirectory|path|manifest|backupRef|hash/i);
  assert.match(saveManagerCss, /\.profile-auto-backup-card/);
});

test("plain browser preview renders the redesigned profiles console instead of the error shell", () => {
  const pageSource = readSource("src/features/profiles/ProfilePage.tsx");
  const directorySource = readSource("src/features/profiles/SaveDirectoryPanel.tsx");

  assert.match(pageSource, /PREVIEW_PROFILES/);
  assert.match(pageSource, /function isPlainBrowserRuntime/);
  assert.match(pageSource, /!"__TAURI_INTERNALS__" in window|!\("__TAURI_INTERNALS__" in window\)/);
  assert.match(pageSource, /createPreviewProfiles\(\)/);
  assert.match(pageSource, /data-preview-mode/);
  assert.match(pageSource, /setProfileState\(\{\s*status:\s*"ready",\s*profiles:\s*previewProfiles\s*\}\)/);
  assert.match(pageSource, /setSettingsState\(\{\s*status:\s*"ready",\s*settings\s*\}\)/);
  assert.match(directorySource, /previewMode\?:\s*boolean/);
  assert.match(directorySource, /if \(previewMode\)/);
  assert.match(directorySource, /Steam\/userdata\/<steam-id>\/582010\/remote/);
});

test("profile save discovery uses a floating notice and candidate confirmation UI", () => {
  const app = readSource("src/App.tsx");
  const main = readSource("src/main.tsx");
  const page = readSource("src/features/profiles/ProfilePage.tsx");
  const panel = readSource("src/features/profiles/SaveDirectoryPanel.tsx");
  const notice = readSource("src/features/profiles/ProfileSaveDirectoryFloatingNotice.tsx");
  const candidates = readSource("src/features/profiles/ProfileSaveDirectoryCandidateList.tsx");
  const css = readSource("src/features/profiles/ProfileSaveDirectoryDiscovery.css");

  assert.match(app, /ProfileSaveDirectoryDiscoveryProvider/);
  assert.match(main, /ProfileSaveDirectoryDiscovery\.css/);
  assert.match(page, /ProfileSaveDirectoryCandidateList/);
  assert.match(panel, /自动检测/);
  assert.match(notice, /positioned by CSS/);
  assert.match(notice, /window\.setTimeout/);
  assert.match(notice, /AUTO_DISMISS_TIMEOUT_MS\s*=\s*6000/);
  assert.match(candidates, /accountName/);
  assert.match(candidates, /avatarUrl/);
  assert.match(candidates, /recommended/);
  assert.match(css, /\.profile-save-directory-floating-notice\s*\{[\s\S]*?position:\s*fixed/);
  assert.match(css, /\.profile-save-directory-floating-notice\s*\{[\s\S]*?top:\s*clamp\(72px,\s*14vh,\s*128px\)/);
  assert.match(css, /\.profile-save-directory-floating-notice\s*\{[\s\S]*?left:\s*50%/);
  assert.match(css, /\.profile-save-directory-floating-notice\s*\{[\s\S]*?transform:\s*translateX\(-50%\)/);
  assert.doesNotMatch(page + panel + notice + candidates, forbiddenDiscoveryFields);
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
  assert.match(panel, /primaryAction=\{!hasDiscoveryCandidates\}/);
});
