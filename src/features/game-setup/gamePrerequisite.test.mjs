import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("game prerequisite API uses a narrow backend command", () => {
  const api = readSource("src/features/game-setup/gamePrerequisiteApi.ts");
  const types = readSource("src/features/game-setup/gamePrerequisiteTypes.ts");
  const hook = readSource("src/features/game-setup/useGamePrerequisites.ts");

  assert.match(api, /invoke<\s*GamePrerequisiteReportDto\s*>\("get_game_prerequisite_status",\s*\{\s*gameId\s*\}\)/);
  assert.doesNotMatch(api, /nativePC|loader-config|CRCBypass|Stracker/i);
  assert.match(types, /"missing"[\s\S]*"misconfigured"[\s\S]*"installed_verified"[\s\S]*"installed_unverified"/);
  assert.match(hook, /getGamePrerequisiteStatus\(gameId\)/);
  assert.match(hook, /status:\s*"loading"/);
});

test("dashboard and settings render the shared prerequisite panel", () => {
  const dashboard = readSource("src/features/dashboard/DashboardPage.tsx");
  const hero = readSource("src/features/dashboard/DashboardHeroCard.tsx");
  const settings = readSource("src/features/settings/SettingsPage.tsx");
  const panel = readSource("src/features/game-setup/GamePrerequisitePanel.tsx");
  const css = readSource("src/features/game-setup/GamePrerequisitePanel.css");

  assert.match(dashboard, /useGamePrerequisites/);
  assert.match(hero, /<GamePrerequisitePanel/);
  assert.match(settings, /<GamePrerequisitePanel/);
  assert.match(panel, /installed_unverified/);
  assert.match(panel, /role="status"/);
  assert.match(css, /\.game-prerequisite-panel/);
  assert.match(css, /\.game-prerequisite-item\.is-warning/);
});
