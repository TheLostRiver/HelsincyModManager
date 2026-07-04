import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("dashboard setup rail advances the highlighted step from game setup status", () => {
  const panelSource = readSource("src/features/dashboard/SetupStatusPanel.tsx");
  const dataSource = readSource("src/features/dashboard/dashboardData.ts");

  assert.doesNotMatch(dataSource, /active:\s*true/);
  assert.match(panelSource, /const activeStepIndex = resolveActiveSetupStepIndex\(status\)/);
  assert.match(panelSource, /isActive=\{index === activeStepIndex\}/);
  assert.match(panelSource, /function resolveActiveSetupStepIndex\(status: GameSetupStatus\)/);
  assert.match(panelSource, /status\.kind === "configured"[\s\S]*return 3;/);
  assert.match(panelSource, /stepLabel:\s*"第 4 \/ 4 步"/);
});
