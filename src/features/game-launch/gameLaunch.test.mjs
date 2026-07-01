import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import test from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("game launch API exposes a narrow launch_game command without path inputs", () => {
  assert.equal(existsSync("src/features/game-launch/gameLaunchApi.ts"), true);
  assert.equal(existsSync("src/features/game-launch/useGameLaunch.ts"), true);

  const api = readSource("src/features/game-launch/gameLaunchApi.ts");
  const types = readSource("src/features/game-launch/gameLaunchTypes.ts");
  const hook = readSource("src/features/game-launch/useGameLaunch.ts");

  assert.match(api, /invoke<[^>]*GameLaunchReceiptDto[^>]*>\("launch_game",\s*\{\s*gameId\s*\}\)/);
  assert.doesNotMatch(api, /directory|path|steam:\/\/|open\(|convertFileSrc|asset/);
  assert.match(types, /GameLaunchMethod\s*=\s*"steam_protocol"/);
  assert.match(types, /GameLaunchErrorCode/);
  assert.match(hook, /launchGame\(gameId\)/);
  assert.match(hook, /game_not_configured/);
});

test("dashboard and app header expose controlled game launch actions", () => {
  const dashboard = readSource("src/features/dashboard/DashboardPage.tsx");
  const hero = readSource("src/features/dashboard/DashboardHeroCard.tsx");
  const header = readSource("src/app/frame/AppHeader.tsx");

  assert.match(dashboard, /useGameLaunch\("mhw"\)/);
  assert.match(hero, /onLaunchGame/);
  assert.match(hero, /启动游戏/);
  assert.match(header, /useGameLaunch\("mhw"\)/);
  assert.match(header, /aria-label=\{isLaunchingGame \? "正在启动游戏" : "启动游戏"\}/);
});
