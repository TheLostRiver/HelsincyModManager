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

test("startup game setup failures surface in-page instead of an auto-opened modal", () => {
  /*
   * 启动检测失败曾经自动弹出模态。它提供的标题、文案与两个操作在工作台的页头、
   * Hero 卡片和设置状态面板里都已存在——模态只贡献阻塞；而且 dismiss 只清组件本地 state，
   * 离开工作台再回来必然重弹，事实上关不掉。组件已整体移除。
   */
  assert.equal(existsSync("src/features/game-setup/GameSetupDialog.tsx"), false);
  assert.equal(existsSync("src/features/game-setup/GameSetupDialog.css"), false);

  const dashboard = readSource("src/features/dashboard/DashboardPage.tsx");
  assert.doesNotMatch(dashboard, /GameSetupDialog/);
  assert.doesNotMatch(dashboard, /dismissStartupNotice/);

  /*
   * 模态独有的诊断细节不得随组件一起丢失：它区分「扫到候选但校验未通过」与
   * 「根本没扫到 Steam 目录」，两者该做的事完全不同。改为交给设置状态面板常驻展示。
   */
  assert.match(dashboard, /startupNotice=\{gameSetup\.startupNotice\}/);
  const panel = readSource("src/features/dashboard/SetupStatusPanel.tsx");
  assert.match(panel, /startupNotice: GameSetupStartupNotice \| null/);
  // notice 只带语义 kind，detail 文本由面板按当前 locale 取。
  assert.match(panel, /deriveStartupDetail\(startupNotice, setupErrors\)/);
  assert.match(panel, /startupDetail \? <p className="state-detail">\{startupDetail\}<\/p> : null/);

  // 推导逻辑保留在 hook 中，只是不再驱动模态。
  const hook = readSource("src/features/game-setup/useGameSetup.ts");
  assert.match(hook, /setStartupNoticeForDetection/);
  assert.doesNotMatch(hook, /dismissStartupNotice/);
});
