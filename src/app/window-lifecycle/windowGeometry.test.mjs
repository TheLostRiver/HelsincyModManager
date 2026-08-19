import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { cwd } from "node:process";
import { test } from "node:test";

const repoRoot = cwd();

function readProjectFile(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

/**
 * 从 CSS 里取出某个选择器在指定 media query 下的断点数值，避免测试与样式各写一份常量。
 */
function asideCollapseBreakpoint() {
  const css = readProjectFile("src/app/routing/RouterOutlet.css");
  const match = /@media\s*\(max-width:\s*(\d+)px\)\s*\{[^}]*\.route-transition__layer/.exec(css);
  assert.ok(match, "RouterOutlet.css 应有一个收起 route aside 的 max-width 断点");
  return Number(match[1]);
}

function defaultWindowConfig() {
  const config = JSON.parse(readProjectFile("src-tauri/tauri.conf.json"));
  const windows = config.app?.windows;
  assert.ok(Array.isArray(windows) && windows.length === 1, "应只有一个默认窗口定义");
  return windows[0];
}

test("默认窗口开箱即高于 route aside 断点", () => {
  const window = defaultWindowConfig();
  const breakpoint = asideCollapseBreakpoint();

  // 默认窗口若落在断点之下，用户一启动就看不到右侧状态栏，且必须最大化才能恢复。
  // 这正是 0.1.0-alpha.0 真机验收发现的缺陷：1200 宽的默认窗口低于当时的 1360 断点。
  assert.ok(
    window.width > breakpoint,
    `默认窗口宽 ${window.width} 必须大于 aside 断点 ${breakpoint}`,
  );
});

test("默认窗口声明溢出保护与居中", () => {
  const window = defaultWindowConfig();

  // tao 建窗时只按 min/max 约束钳制，不看显示器工作区。没有 preventOverflow，
  // 920 的默认高度在 1920x1080@125%（逻辑 1536x864）与 1600x900 上会超出屏幕。
  // 它是唯一的防溢出机制，必须有回归门禁。
  assert.equal(window.preventOverflow, true, "必须启用 preventOverflow");
  assert.equal(window.center, true, "必须居中，否则钳制后的窗口可能贴在角落");
});

test("窗口最小尺寸不剥夺小屏与分屏使用", () => {
  const window = defaultWindowConfig();

  // minWidth 不得为了迁就断点而抬高：860/640/560 三档窄屏降级是受支持且被
  // 契约测试锁住的路径，1280 逻辑宽（1920x1080@150%）与并排查攻略的分屏用法
  // 都必须可用。
  assert.ok(window.minWidth <= 1024, `minWidth ${window.minWidth} 不应超过 1024`);
  assert.ok(window.minHeight <= 640, `minHeight ${window.minHeight} 不应超过 640`);
});
