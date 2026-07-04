import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import test from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

function getRuleBody(css, selector) {
  const start = css.indexOf(`${selector} {`);
  assert.ok(start >= 0, `missing CSS rule: ${selector}`);

  const openBraceIndex = css.indexOf("{", start);
  const closeBraceIndex = css.indexOf("}", openBraceIndex);
  assert.ok(openBraceIndex >= 0 && closeBraceIndex > openBraceIndex, `invalid CSS rule: ${selector}`);
  return css.slice(openBraceIndex + 1, closeBraceIndex);
}

test("game setup startup auto detection uses a narrow backend command and persists valid discoveries", () => {
  const api = readSource("src/features/game-setup/gameSetupApi.ts");
  const types = readSource("src/features/game-setup/gameSetupTypes.ts");
  const hook = readSource("src/features/game-setup/useGameSetup.ts");
  const sharedApi = readSource("src/shared/api/tauri.ts");

  assert.match(api, /autoDetectGameDirectory/);
  assert.match(api, /invoke<[^>]*GameAutoDetectionDto[^>]*>\("auto_detect_game_directory",\s*\{\s*gameId\s*\}\)/);
  assert.doesNotMatch(api, /MonsterHunterWorld\.exe|nativePC|steamapps|libraryfolders|appmanifest/);
  assert.match(types, /GameAutoDetectionOutcome[\s\S]*"already_configured"[\s\S]*"detected_and_saved"[\s\S]*"not_found"[\s\S]*"invalid_candidate"[\s\S]*"scan_failed"/);
  assert.match(hook, /from "\.\/gameSetupApi"/);
  assert.match(hook, /autoDetectGameDirectory\(gameId\)/);
  assert.match(hook, /setStartupNoticeForDetection/);
  assert.match(hook, /startupNotice/);
  assert.match(sharedApi, /autoDetectGameDirectory/);
});

test("dashboard renders startup game setup failures as a centered floating notice", () => {
  assert.equal(existsSync("src/features/game-setup/GameSetupFloatingNotice.tsx"), true);
  assert.equal(existsSync("src/features/game-setup/GameSetupFloatingNotice.css"), true);

  const dashboard = readSource("src/features/dashboard/DashboardPage.tsx");
  const notice = readSource("src/features/game-setup/GameSetupFloatingNotice.tsx");
  const css = readSource("src/features/game-setup/GameSetupFloatingNotice.css");

  assert.match(dashboard, /<GameSetupFloatingNotice/);
  assert.match(dashboard, /notice=\{gameSetup\.startupNotice\}/);
  assert.match(dashboard, /onRetry=\{gameSetup\.retryStartupDetection\}/);
  assert.match(dashboard, /onDismiss=\{gameSetup\.dismissStartupNotice\}/);

  assert.match(notice, /className="game-setup-floating-notice"/);
  assert.match(notice, /role="status"/);
  assert.match(notice, /aria-live="polite"/);
  assert.match(notice, /onRetry/);
  assert.match(notice, /onManualSelect/);
  assert.match(notice, /onDismiss/);
  assert.doesNotMatch(notice, /className=".*banner|role="dialog"|aria-modal="true"/);

  const body = getRuleBody(css, ".game-setup-floating-notice");
  assert.match(body, /position:\s*fixed;/);
  assert.match(body, /top:\s*clamp\(72px,\s*14vh,\s*128px\);/);
  assert.match(body, /left:\s*50%;/);
  assert.match(body, /transform:\s*translateX\(-50%\);/);
  assert.match(body, /z-index:\s*80;/);
  assert.doesNotMatch(body, /position:\s*static|position:\s*sticky/);
});

test("startup floating notice auto dismisses after a short idle window", () => {
  const notice = readSource("src/features/game-setup/GameSetupFloatingNotice.tsx");

  assert.match(notice, /AUTO_DISMISS_TIMEOUT_MS\s*=\s*6000/);
  assert.match(notice, /useEffect/);
  assert.match(notice, /window\.setTimeout\(\(\)\s*=>\s*onDismiss\(\),\s*AUTO_DISMISS_TIMEOUT_MS\)/);
  assert.match(notice, /window\.clearTimeout\(dismissTimer\)/);
});
