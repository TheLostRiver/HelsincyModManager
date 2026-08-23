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

test("dashboard exposes launch while app header stays status-only", () => {
  const dashboard = readSource("src/features/dashboard/DashboardPage.tsx");
  const hero = readSource("src/features/dashboard/DashboardHeroCard.tsx");
  const dashboardCss = readSource("src/features/dashboard/Dashboard.css");
  const header = readSource("src/app/frame/AppHeader.tsx");

  assert.match(dashboard, /useGameLaunch\("mhw"\)/);
  assert.match(hero, /onLaunchGame/);
  assert.match(hero, /heroCopyDict\.hero\.launching : heroCopyDict\.hero\.launchButton/);
  // zh 值 pin 移到 copy 模块。
  const heroCopySource = readSource("src/features/dashboard/dashboardCopy.ts");
  assert.match(heroCopySource, /launchButton: "启动游戏"/);
  assert.match(heroCopySource, /launching: "正在启动"/);
  assert.match(hero, /className=\{`launch-action-card\$\{isLaunchReady \? "" : " is-disabled"\}`\}/);
  assert.match(hero, /role="group"/);
  assert.match(hero, /aria-label=\{heroCopyDict\.hero\.launchGroupAria\}/);
  assert.match(heroCopySource, /launchGroupAria: "游戏启动"/);
  assert.match(hero, /className="launch-action-button"/);
  assert.match(hero, /const isLaunchReady = status\.kind === "configured"/);
  assert.match(hero, /disabled=\{!isLaunchReady \|\| launchState\.isLaunchingGame\}/);
  assert.match(heroCopySource, /readyDescription: "当前配置档可用，游戏目录已通过校验。"/);
  assert.match(hero, /copy\.blockedDescription/);
  assert.equal(heroCopySource.match(/配置游戏目录后即可启动。/g)?.length, 1);
  assert.match(dashboardCss, /\.launch-action-card\s*\{/);
  assert.match(dashboardCss, /\.launch-action-button\s*\{/);
  assert.match(dashboardCss, /\.launch-action-button[\s\S]*?border-radius:\s*var\(--radius-inner\);/);
  assert.doesNotMatch(header, /useGameLaunch\("mhw"\)/);
  assert.doesNotMatch(header, /aria-label=\{isLaunchingGame \? "正在启动游戏" : "启动游戏"\}/);
});
