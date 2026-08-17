import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const read = (path) => readFileSync(path, "utf8");

test("backup center is an enabled first-class route with tracked feature-local sources", () => {
  const routeTypes = read("src/app/routing/routeTypes.ts");
  const routes = read("src/app/routing/routeRegistry.tsx");
  const nav = read("src/app/shell/navigation/navItems.ts");
  const main = read("src/main.tsx");
  const gitignore = read(".gitignore");

  assert.match(routeTypes, /"backups"/);
  assert.match(routes, /id:\s*"backups"[\s\S]*?path:\s*"\/backups"[\s\S]*?element:\s*BackupCenterPage/);
  const navLine = nav.split("\n").find((line) => line.includes('id: "backups"'));
  assert.ok(navLine);
  assert.match(navLine, /label:\s*"备份整理"/);
  assert.equal(navLine.includes("disabledReason"), false);
  assert.match(main, /features\/backups\/BackupCenterPage\.css/);
  assert.match(gitignore, /!src\/features\/backups\/\*\*/);
});

test("backup center uses narrow camelCase DTOs and controlled commands", () => {
  const api = read("src/features/backups/backupCenterApi.ts");
  const types = read("src/features/backups/backupCenterTypes.ts");

  assert.match(api, /invoke<SaveBackupCenterPageDto>\("query_save_backup_center"/);
  assert.match(api, /invoke<\{ note: string \| null \}>\("update_save_backup_note"/);
  assert.match(api, /invoke<SaveBackupRetentionReportDto>\("run_save_backup_retention"/);
  for (const field of ["profileId", "archiveBytes", "maxTotalBytes", "budgetSatisfied", "releasedBytes", "evidenceDegraded"]) {
    assert.match(types, new RegExp(`${field}:`));
  }
  assert.match(api, /backupId:\s*string/);
  for (const forbidden of ["backupDirectory", "archivePath", "manifestPath", "steamId", "readFile", "convertFileSrc"]) {
    assert.equal(api.includes(forbidden), false, `${forbidden} must stay outside the backup center API`);
  }
});

test("backup center keeps restore controlled and exposes note and retention states", () => {
  const page = read("src/features/backups/BackupCenterPage.tsx");

  assert.match(page, /<h1>备份整理<\/h1>/);
  assert.match(page, /跨配置档查看备份历史、保护点与整理状态/);
  assert.match(page, /SaveRestoreDialog/);
  assert.match(page, /backup\.status === "completed"\s*\?/);
  assert.match(page, /恢复存档/);
  assert.match(page, /updateSaveBackupNote/);
  assert.match(page, /runSaveBackupRetention/);
  assert.match(page, /role="alertdialog"/);
  assert.match(page, /确认立即整理备份/);
  assert.match(page, /最新普通备份和恢复前保护点不会被删除/);
  assert.match(page, /initialFocusRef=\{cancelMaintenanceRef\}/);
  assert.match(page, /report\.outcome === "partial"/);
  assert.match(page, /report\.outcome === "blocked"/);
  assert.match(page, /report\.outcome === "failed"/);
  assert.match(page, /report\.evidenceDegraded/);
  assert.match(page, /审计记录不可用/);
  assert.match(page, /profile\.retention\.maxCount === 0\s*\? 0/);
  assert.match(page, /pageState\.status === "error"/);
  assert.match(page, /retention_partial/);
  assert.match(page, /retention_pending/);
  assert.match(page, /profile\.steamAccount\?\.avatarUrl/);
  assert.match(page, /TRUSTED_AVATAR_HOSTS/);
  assert.match(page, /url\.protocol !== "https:"/);
  assert.match(page, /<img[\s\S]*?onError=/);
  assert.match(page, /referrerPolicy="no-referrer"/);
  assert.match(page, /maxLength=\{100\}/);
  assert.match(page, /function lastValidPageOffset/);
  assert.match(page, /pageState\.page\.offset > normalizedOffset/);
  assert.match(page, /aria-label="备份备注"/);
});

test("backup center is responsive without horizontal scrolling or translucent blur surfaces", () => {
  const css = read("src/features/backups/BackupCenterPage.css");
  const reducedMotion = css.slice(css.indexOf("@media (prefers-reduced-motion: reduce)"));

  assert.match(css, /\.backup-center-page\s*\{[\s\S]*?overflow-x:\s*hidden/);
  assert.match(css, /@media\s*\(max-width:\s*520px\)/);
  assert.match(css, /@media\s*\(prefers-reduced-motion:\s*reduce\)/);
  assert.match(reducedMotion, /\.backup-spin,[\s\S]*?animation:\s*none/);
  assert.doesNotMatch(css, /overflow-x:\s*auto/);
  assert.doesNotMatch(css, /backdrop-filter|filter:\s*blur/);
  assert.doesNotMatch(css, /min-width:\s*[6-9]\d{2}px/);
  assert.match(css, /\.backup-profile-avatar img\s*\{[\s\S]*?object-fit:\s*cover/);
});

test("backup restore action preserves primary text color while hovered", () => {
  const css = read("src/features/backups/BackupCenterPage.css");
  const hoverRule = css.match(/\.backup-action-button\.is-primary:hover:not\(:disabled\),[\s\S]*?\.backup-action-button\.is-restore:hover:not\(:disabled\)\s*\{[^}]*\}/)?.[0] ?? "";

  assert.match(hoverRule, /color:\s*var\(--color-primary-action-text\)/);
  assert.match(hoverRule, /background:\s*var\(--color-primary-action-bg-hover\)/);
  assert.match(hoverRule, /border-color:\s*var\(--color-primary-action-border\)/);
});
