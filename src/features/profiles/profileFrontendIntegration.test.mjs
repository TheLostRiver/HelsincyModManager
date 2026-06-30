import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("profiles route is registered and enabled from the shared navigation definition", () => {
  assert.equal(existsSync("src/features/profiles/ProfilePage.tsx"), true);
  assert.equal(existsSync("src/features/profiles/ProfilePage.css"), true);

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
  assert.match(source, /setProfileSaveSettings/);
  assert.match(source, /setActiveProfile/);
  assert.match(source, /refreshActiveProfile/);
  assert.doesNotMatch(source, /useSidebarMode|sidebarMode/);
  assert.doesNotMatch(source, /manifestPath|backupRoot|backupRef|targetPath|sandbox|cache/i);
  assert.match(css, /\.route-transition__layer\[data-route-id="profiles"\]/);
  assert.match(css, /\.profile-workspace/);
  assert.match(css, /\.profile-settings-panel/);
  assert.match(css, /\.profile-directory-row/);
  assert.doesNotMatch(css, /profile-page__summary-grid|profile-main-card|profile-row/);
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
  assert.match(pickerSource, /onWheel=\{handleWheel\}/);
  assert.match(pickerSource, /onPointerMove=\{handlePointerMove\}/);
  assert.match(pickerSource, /weekdayOrder/);
  assert.match(viewModelSource, /weekdays:\s*\[1\]/);
  assert.match(viewModelSource, /\[0,\s*"星期日"\]/);
  assert.match(css, /perspective:\s*420px/);
  assert.match(css, /transform-style:\s*preserve-3d/);
});
