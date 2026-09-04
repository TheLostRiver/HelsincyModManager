import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

/*
 * #333：已配置游戏目录时，手动选择一个校验失败的目录，曾经把 status 从 configured
 * 拍成 invalid——UI 显示配置丢失，并连带禁用一键启动、恢复中心、安装健康检测。
 * 后端其实原封不动（game_setup.rs 的 save_game_directory 所有校验都先于落盘），
 * 所以这是纯前端的状态机缺陷。
 *
 * 修复取向：status 描述「当前配置状态」，「上次保存失败」是另一个维度，
 * 两者必须分开存。以下断言锁住这个分离。
 */

test("save failure keeps the configured status instead of rewriting it to invalid", () => {
  const hook = readSource("src/features/game-setup/useGameSetup.ts");

  /*
   * 核心断言：catch 分支里 status 的取值必须先看现状。写成无条件
   * { kind: "invalid" } 就是 #333 的原始形态。
   */
  assert.match(
    hook,
    /current\.status\.kind === "configured"\s*\n\s*\? current\.status\s*\n\s*:\s*\{\s*\n\s*kind: "invalid"/,
    "saveDirectory 的 catch 必须在已配置时保留 status，仅在未配置时转 invalid",
  );

  // 失败原因改由正交字段承载。
  assert.match(hook, /lastSaveError: mapped\.code/);
});

test("lastSaveError is a field on the state, orthogonal to status", () => {
  const hook = readSource("src/features/game-setup/useGameSetup.ts");

  assert.match(hook, /lastSaveError: GameSetupErrorCode \| null/);
  // 初值为 null：启动阶段不该凭空出现失败提示。
  assert.match(hook, /lastSaveError: null,\s*\n\s*isBusy: false/);

  // 保存成功必须清掉残留的上次失败。
  const successBranch = hook.slice(hook.indexOf("const dto = await saveGameDirectory"));
  assert.match(successBranch.slice(0, 400), /lastSaveError: null/);

  // 读到已配置同样清掉：那证明失败已经不是现状。
  const refreshBranch = hook.slice(hook.indexOf("const refresh = useCallback"));
  assert.match(
    refreshBranch.slice(0, 900),
    /lastSaveError: dto\.kind === "configured" \? null : current\.lastSaveError/,
  );
});

test("the setup panel renders a persistent save failure while configured", () => {
  const panel = readSource("src/features/dashboard/SetupStatusPanel.tsx");

  assert.match(panel, /lastSaveError: GameSetupErrorCode \| null/);
  /*
   * 关键：configured 分支原本完全不消费 actionMessage（已 grep 确认），
   * 所以失败原因必须走独立字段，不能指望塞进 actionMessage 就能显示。
   */
  assert.match(panel, /status\.kind === "configured" && lastSaveError/);
  assert.match(panel, /messageForError\(lastSaveError, setupErrors\.errors\)/);
  // 常驻显示，且对读屏软件是可感知的告警；配色沿用 danger token。
  assert.match(panel, /saveErrorDetail \? \(/);
  assert.match(panel, /className="state-detail is-error" role="alert"/);

  /*
   * 它描述「上次操作的结果」，与 startupNotice（启动自动检测的失败）是两件事，
   * 因此各自独立渲染、不互相覆盖——曾经为了加 role 属性把它俩合并成
   * stateDetail，那会让手动保存的失败顶掉启动检测的说明。
   */
  assert.match(panel, /startupDetail \? <p className="state-detail">/);
  assert.doesNotMatch(panel, /const stateDetail =/);
});

test("the persistent save failure reuses the danger colour tokens", () => {
  const css = readSource("src/features/dashboard/Dashboard.css");
  assert.match(css, /\.current-state \.state-detail\.is-error \{[^}]*--color-danger-text/);
  assert.match(css, /\.current-state \.state-detail\.is-error \{[^}]*--color-danger-bg/);
  assert.match(css, /\.current-state \.state-detail\.is-error \{[^}]*--color-danger-dot/);
});

test("the dashboard passes lastSaveError down to the setup panel", () => {
  const dashboard = readSource("src/features/dashboard/DashboardPage.tsx");
  assert.match(dashboard, /lastSaveError=\{gameSetup\.lastSaveError\}/);
});

/*
 * 安全底线：本 bug 之所以只是前端问题，是因为后端在落盘之前就拒绝了。
 * 任何人「顺手」把校验挪到 save_game_instance 之后，这个缺陷就会从显示错误
 * 升级成真的覆盖玩家配置——值得单独锁一条。
 */
test("backend still validates before it persists the game directory", () => {
  const service = readSource("src-tauri/crates/hmm-app/src/game_setup.rs");
  const saveFn = service.slice(service.indexOf("pub fn save_game_directory"));

  const validationAt = saveFn.indexOf("validate_directory");
  const overlapAt = saveFn.indexOf("OverlapsModStorage");
  const persistAt = saveFn.indexOf("save_game_instance");

  assert.ok(validationAt >= 0, "应存在目录校验");
  assert.ok(overlapAt >= 0, "应存在存储目录重叠校验");
  assert.ok(persistAt >= 0, "应存在落盘调用");
  assert.ok(
    validationAt < persistAt,
    "目录校验必须先于落盘，否则校验失败会覆盖玩家已有配置",
  );
  assert.ok(
    overlapAt < persistAt,
    "存储目录重叠校验必须先于落盘，否则 #333 会从显示错误升级为数据覆盖",
  );
});
