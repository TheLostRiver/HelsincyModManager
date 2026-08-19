import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { cwd } from "node:process";
import { test } from "node:test";

const repoRoot = cwd();

function readSource(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

test("顶部状态栏读真实游戏目录状态而不是写死文案", () => {
  const header = readSource("src/app/frame/AppHeader.tsx");

  // 改动前这三处都是写死的 JSX 字符串，游戏目录明明已配置，顶栏仍显示"目录未配置"。
  assert.match(header, /useGameSetup\(\)/);
  assert.doesNotMatch(
    header,
    /<strong>目录未配置<\/strong>/,
    "目录状态不得是写死的字面量",
  );
  assert.match(header, /gameSetupStatus\.kind === "configured"/);

  // 四种状态都要有对应文案，否则未覆盖的分支会静默显示成错误状态。
  for (const kind of ["configured", "validating", "invalid", "not_configured"]) {
    assert.match(header, new RegExp(`case "${kind}":`), `目录状态缺少 ${kind} 分支`);
  }
});

test("目录状态徽章用到的每种色调都有样式", () => {
  const header = readSource("src/app/frame/AppHeader.tsx");
  const css = readSource("src/app/frame/AppFrame.css");

  // 徽章的 class 是拼出来的（`status-pill ${tone}`），少一种色调不会报错，
  // 只会渲染成没有背景的裸文字。
  const tones = [...header.matchAll(/tone:\s*"(\w+)"/g)].map((match) => match[1]);
  assert.ok(tones.length >= 4, "应覆盖四种目录状态色调");

  for (const tone of new Set(tones)) {
    assert.match(css, new RegExp(`\\.status-pill\\.${tone}\\s*\\{`), `缺少 .status-pill.${tone} 样式`);
    assert.match(css, new RegExp(`\\.${tone}-dot\\s*\\{`), `缺少 .${tone}-dot 样式`);
  }
});

test("游戏目录状态全应用共享一个实例", () => {
  const provider = readSource("src/features/game-setup/GameSetupProvider.tsx");
  const app = readSource("src/App.tsx");
  const stateHook = readSource("src/features/game-setup/useGameSetup.ts");

  // 状态 hook 在挂载时会触发启动自检（含 Steam 库扫描与 10 秒超时）。
  // 改动前三个组件各自调用它，自检按调用方数量重复执行，且各方持有独立副本——
  // 在工作台配置完目录，顶栏不会更新。
  assert.match(stateHook, /export function useGameSetupState/);
  assert.match(provider, /useGameSetupState\(gameId\)/);
  assert.match(app, /<GameSetupProvider>/);

  // 除 provider 外不得再有人直接调用状态 hook。
  const directCallers = ["src/features/dashboard/DashboardPage.tsx",
    "src/features/install-recovery/RecoveryCenterPage.tsx",
    "src/features/install-recovery/InstallRecoveryGlobalAlertPanel.tsx"];
  for (const path of directCallers) {
    const source = readSource(path);
    assert.doesNotMatch(source, /useGameSetupState/, `${path} 不应直接调用状态 hook`);
    assert.match(source, /from "\.\.\/game-setup\/GameSetupProvider"/);
  }

  // 缺少 provider 时必须显式报错，而不是静默拿到空状态。
  assert.match(provider, /throw new Error/);
});
