import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

async function importTypeScriptModule(path) {
  const source = readFileSync(path, "utf8");
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ES2022,
      target: ts.ScriptTarget.ES2022,
    },
  });
  const moduleUrl = `data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`;
  return import(moduleUrl);
}

test("dashboard setup rail advances the highlighted step from game setup status", async () => {
  const { resolveActiveSetupStepIndex, resolveSetupSteps } = await importTypeScriptModule(
    "src/features/dashboard/setupStatusSteps.ts",
  );
  const { dashboardCopy } = await importTypeScriptModule(
    "src/features/dashboard/dashboardCopy.ts",
  );
  // 功能测试固定使用 zh_cn 字典，断言中文步骤标题不回归。
  const zhSteps = dashboardCopy.zh_cn.steps;

  const cases = [
    [{ kind: "not_configured", gameId: "mhw" }, 0, "扫描 Steam 游戏库"],
    [{ kind: "validating", gameId: "mhw" }, 1, "验证游戏目录"],
    [
      { kind: "invalid", gameId: "mhw", errorCode: "missing_executable", message: "missing executable" },
      1,
      "验证游戏目录",
    ],
    [
      {
        kind: "configured",
        gameId: "mhw",
        displayName: "Monster Hunter: World - Iceborne",
        pathLabel: ".../Monster Hunter World",
      },
      3,
      "开始导入模组",
    ],
  ];

  for (const [status, expectedIndex, expectedTitle] of cases) {
    assert.equal(resolveActiveSetupStepIndex(status), expectedIndex);

    const steps = resolveSetupSteps(status, zhSteps);
    assert.equal(steps.length, 4);
    assert.equal(steps[expectedIndex].title, expectedTitle);
    assert.deepEqual(
      steps.map((step) => step.isActive),
      steps.map((_, index) => index === expectedIndex),
    );
  }
});
