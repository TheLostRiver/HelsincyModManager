import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

/*
 * #333 的跨文件契约。状态机本身的行为断言在 gameSetupSaveState.test.mjs——那里能真正
 * 驱动迁移序列，是本缺陷的主防线。
 *
 * 本文件只留三条单测覆盖不到的：两条是「A 文件的字段有没有接到 B 文件」的连线，
 * 一条是后端源码里的语句顺序。它们仍是源码断言，因为断言对象本就是源码结构而非行为。
 *
 * 历史教训：本文件曾用同一手法断言 useGameSetup.ts 里「存在某段三元表达式」，
 * 以此证明修复到位。那段表达式确实存在，但它读的 current.status 早已被同一函数开头的
 * 乐观转场写成 validating，判断恒假——缺陷带着绿灯合并进 main（#333 第一版）。
 * 源码断言只能锁结构，永远不要用它冒充行为验证。
 */

test("the dashboard wires the save failure through to the setup panel", () => {
  /*
   * lastSaveError 是一条跨三个文件的连线：hook 产出 → DashboardPage 传递 →
   * SetupStatusPanel 渲染。断了任一环，状态机再对也显示不出来。
   */
  const dashboard = readSource("src/features/dashboard/DashboardPage.tsx");
  assert.match(dashboard, /lastSaveError=\{gameSetup\.lastSaveError\}/);

  const panel = readSource("src/features/dashboard/SetupStatusPanel.tsx");
  assert.match(panel, /lastSaveError: GameSetupErrorCode \| null/);
  /*
   * configured 分支原本完全不消费 actionMessage（已 grep 确认），所以失败原因必须
   * 走独立字段，不能指望塞进 actionMessage 就能显示。
   */
  assert.match(panel, /status\.kind === "configured" && lastSaveError/);
  assert.match(panel, /messageForError\(lastSaveError, setupErrors\.errors\)/);
  // 常驻显示（不随 toast 消失），且对读屏软件是可感知的告警。
  assert.match(panel, /className="state-detail is-error" role="alert"/);

  /*
   * 它描述「上次操作的结果」，与 startupNotice（启动自动检测的失败）是两件事，
   * 因此各自独立渲染、不互相覆盖——曾经为了加 role 属性把它俩合并成 stateDetail，
   * 那会让手动保存的失败顶掉启动检测的说明。
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

/*
 * 安全底线：本缺陷之所以只是显示错误，是因为后端在落盘之前就拒绝了。任何人「顺手」
 * 把校验挪到 save_game_instance 之后，它就会从显示错误升级成真的覆盖玩家配置。
 * 语句顺序只能在源码上断言，值得单独锁一条。
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
