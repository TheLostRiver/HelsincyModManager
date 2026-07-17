import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import test from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
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
  assert.match(hook, /withTimeout\(\s*autoDetectGameDirectory\(gameId\)/);
  assert.match(hook, /STARTUP_DETECTION_TIMEOUT_MS\s*=\s*10000/);
  assert.match(hook, /mapped\.code === "unknown"[\s\S]*current\.status[\s\S]*kind: "invalid"/);
  assert.match(hook, /setStartupNoticeForDetection/);
  assert.match(hook, /detection\.errorCode \?\? \(detection\.outcome === "scan_failed" \? "scan_failed" : "directory_not_found"\)/);
  assert.match(hook, /startupNotice/);
  assert.match(sharedApi, /autoDetectGameDirectory/);
});

test("dashboard renders startup game setup failures in the shared modal dialog", () => {
  assert.equal(existsSync("src/features/game-setup/GameSetupDialog.tsx"), true);
  assert.equal(existsSync("src/features/game-setup/GameSetupDialog.css"), true);

  const dashboard = readSource("src/features/dashboard/DashboardPage.tsx");
  const dialog = readSource("src/features/game-setup/GameSetupDialog.tsx");
  const modal = readSource("src/shared/feedback/ModalSurface.tsx");

  assert.match(dashboard, /<GameSetupDialog/);
  assert.match(dashboard, /notice=\{gameSetup\.startupNotice\}/);
  assert.match(dashboard, /onRetry=\{gameSetup\.retryStartupDetection\}/);
  assert.match(dashboard, /onDismiss=\{gameSetup\.dismissStartupNotice\}/);

  assert.match(dialog, /<Dialog/);
  assert.match(dialog, /closeOnEscape=\{!isBusy\}/);
  assert.match(dialog, /closeOnBackdrop=\{!isBusy\}/);
  assert.match(dialog, /busy=\{isBusy\}/);
  assert.match(dialog, /initialFocusRef=\{retryButtonRef\}/);
  assert.match(dialog, /onRetry/);
  assert.match(dialog, /onManualSelect/);
  assert.match(dialog, /onDismiss/);
  assert.match(modal, /role=\{role\}/);
  assert.match(modal, /aria-modal="true"/);
});

test("startup game setup dialog requires an explicit close or successful setup", () => {
  const dialog = readSource("src/features/game-setup/GameSetupDialog.tsx");

  assert.doesNotMatch(dialog, /AUTO_DISMISS_TIMEOUT_MS|setTimeout|isDismissPaused/);
  assert.match(dialog, /onClose=\{onDismiss\}/);
  assert.match(dialog, /disabled=\{isBusy\}/);
  assert.match(dialog, /typeof selected === "string"/);
  assert.match(dialog, /notice\.detail\.trim\(\)\s*!==\s*notice\.message\.trim\(\)/);
});
