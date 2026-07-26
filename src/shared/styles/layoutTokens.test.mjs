import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { cwd } from "node:process";
import { test } from "node:test";

const repoRoot = cwd();

function readProjectFile(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

function stripComments(css) {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function readMediaBlockBody(css, query) {
  const normalizedCss = stripComments(css);
  const match = new RegExp(`@media\\s*\\(${escapeRegExp(query)}\\)`).exec(normalizedCss);

  assert.ok(match, `Missing media block: ${query}`);

  const openBraceIndex = normalizedCss.indexOf("{", match.index);
  return readBlock(normalizedCss, openBraceIndex).body;
}

function readBlock(css, openBraceIndex) {
  let depth = 0;

  for (let index = openBraceIndex; index < css.length; index += 1) {
    const char = css[index];

    if (char === "{") {
      depth += 1;
    } else if (char === "}") {
      depth -= 1;

      if (depth === 0) {
        return {
          body: css.slice(openBraceIndex + 1, index),
          endIndex: index + 1,
        };
      }
    }
  }

  throw new Error(`Unclosed CSS block near index ${openBraceIndex}`);
}

function parseSimpleRules(css, media = null) {
  const rules = [];
  let index = 0;

  while (index < css.length) {
    while (/\s/.test(css[index] ?? "")) {
      index += 1;
    }

    if (index >= css.length) {
      break;
    }

    if (css[index] === "@") {
      const atRuleStart = index;
      const nextBrace = css.indexOf("{", index);
      const nextSemicolon = css.indexOf(";", index);

      if (nextSemicolon !== -1 && (nextBrace === -1 || nextSemicolon < nextBrace)) {
        index = nextSemicolon + 1;
        continue;
      }

      if (nextBrace === -1) {
        break;
      }

      const block = readBlock(css, nextBrace);

      if (css.slice(atRuleStart, nextBrace).trimStart().startsWith("@media")) {
        const query = css
          .slice(atRuleStart + "@media".length, nextBrace)
          .trim();
        rules.push(...parseSimpleRules(block.body, query));
      }

      index = block.endIndex;
      continue;
    }

    const nextBrace = css.indexOf("{", index);
    if (nextBrace === -1) {
      break;
    }

    const selector = css.slice(index, nextBrace).trim();
    const block = readBlock(css, nextBrace);

    if (selector) {
      rules.push({ selector, body: block.body, media });
    }

    index = block.endIndex;
  }

  return rules;
}

function parseCssRules(css) {
  return parseSimpleRules(stripComments(css));
}

function findRule(rules, selector, media = null) {
  return rules.find((rule) => rule.selector === selector && rule.media === media);
}

function expectDeclaration(rule, pattern, message) {
  assert.ok(rule, message);
  assert.match(rule.body, pattern, message);
}

// ===== L1: token 存在与硬编码消除 =====

test("tokens.css 定义全部布局 token", () => {
  const tokensCss = readProjectFile("src/shared/styles/tokens.css");

  for (const tokenName of [
    "--layout-shell-max-width",
    "--layout-page-padding",
    "--layout-content-gap",
    "--layout-route-aside-width",
    "--layout-mod-action-panel-width",
    "--layout-mod-card-min-width",
    "--layout-mod-card-poster-height",
    "--layout-text-overflow",
  ]) {
    assert.match(tokensCss, new RegExp(`${tokenName}:`), `缺少 token: ${tokenName}`);
  }
});

test("tokens.css 宽屏断点逐级覆盖 shell max-width", () => {
  const tokensCss = readProjectFile("src/shared/styles/tokens.css");

  assert.match(tokensCss, /--layout-shell-max-width:\s*1920px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*1921px\)\s*{[\s\S]*--layout-shell-max-width:\s*2400px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*2561px\)\s*{[\s\S]*--layout-shell-max-width:\s*3040px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*2561px\)\s*{[\s\S]*--layout-page-padding:\s*32px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*2561px\)\s*{[\s\S]*--layout-content-gap:\s*20px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*2561px\)\s*{[\s\S]*--layout-mod-action-panel-width:\s*200px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*2561px\)\s*{[\s\S]*--layout-mod-card-min-width:\s*208px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*3201px\)\s*{[\s\S]*--layout-shell-max-width:\s*min\(100vw,\s*3440px\);/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*3201px\)\s*{[\s\S]*--layout-page-padding:\s*36px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*3201px\)\s*{[\s\S]*--layout-content-gap:\s*22px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*3201px\)\s*{[\s\S]*--layout-mod-action-panel-width:\s*212px;/);
  assert.match(tokensCss, /@media\s*\(min-width:\s*3201px\)\s*{[\s\S]*--layout-mod-card-min-width:\s*212px;/);
  assert.doesNotMatch(tokensCss, /@media\s*\(min-width:\s*3201px\)\s*{[\s\S]*--layout-shell-max-width:\s*min\(100vw,\s*3200px\);/);
});

test("tokens.css 超宽档位不得覆盖 dashboard route aside token", () => {
  const tokensCss = readProjectFile("src/shared/styles/tokens.css");
  for (const query of ["min-width: 2561px", "min-width: 3201px"]) {
    const blockBody = readMediaBlockBody(tokensCss, query);
    assert.doesNotMatch(
      blockBody,
      /--layout-route-aside-width\s*:/,
      "超宽档位不应覆盖 --layout-route-aside-width",
    );
  }
});

test("tokens.css 宽屏断点必须同时覆盖 light/dark theme root", () => {
  const tokensCss = readProjectFile("src/shared/styles/tokens.css");

  for (const breakpoint of [1921, 2561, 3201]) {
    assert.match(
      tokensCss,
      new RegExp(
        `@media\\s*\\(min-width:\\s*${breakpoint}px\\)\\s*{\\s*:root,\\s*:root\\[data-color-scheme="light"\\],\\s*:root\\[data-color-scheme="dark"\\]\\s*{`,
      ),
      `宽屏断点 ${breakpoint}px 未同时覆盖 light/dark theme root`,
    );
  }
});

test("layout fixture keeps data-color-scheme on html root", () => {
  const fixtureHtml = readProjectFile("src/shared/styles/layout.fixture.html");

  assert.match(fixtureHtml, /<html[^>]*\sdata-color-scheme="light"/);
  assert.doesNotMatch(fixtureHtml, /<body[^>]*\sdata-color-scheme=/);
});

test("AppFrame 不再硬编码 1920px，改为 token", () => {
  const css = readProjectFile("src/app/frame/AppFrame.css");

  assert.doesNotMatch(css, /\.app-shell[\s\S]*?max-width:\s*1920px;/);
  assert.match(css, /\.app-shell[\s\S]*?max-width:\s*var\(--layout-shell-max-width\);/);
  assert.match(css, /\.app-surface[\s\S]*?gap:\s*var\(--layout-content-gap\);/);
  assert.match(css, /\.app-surface[\s\S]*?padding:\s*var\(--layout-page-padding\);/);
});

test("RouterOutlet 与 Dashboard 都消费 route aside token，且无残留 360px 硬编码", () => {
  for (const file of ["src/app/routing/RouterOutlet.css", "src/features/dashboard/Dashboard.css"]) {
    const css = readProjectFile(file);

    assert.match(
      css,
      /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+var\(--layout-route-aside-width\);/,
      `${file} 未 token 化双列`,
    );
  }

  const routerCss = readProjectFile("src/app/routing/RouterOutlet.css");
  const dashCss = readProjectFile("src/features/dashboard/Dashboard.css");

  assert.doesNotMatch(routerCss, /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+360px;/);
  assert.doesNotMatch(dashCss, /\.workbench-body[\s\S]*?grid-template-columns:[^;]*360px;/);
  assert.doesNotMatch(dashCss, /\.setup-rail[\s\S]*?width:\s*360px;/);
});

test("Mod 管理页消费密度 token，无残留硬编码", () => {
  /*
   * Mod 库样式分布在页面骨架与卡片两个文件中，密度 token 的消费与硬编码残留
   * 都要按合并后的样式表检查，否则把规则搬到另一个文件就能绕过本断言。
   */
  const css = [
    readProjectFile("src/features/mods/ModPosterCard.css"),
    readProjectFile("src/features/mods/ModLibraryPage.css"),
  ].join("\n");

  // 卡片网格密度仍由 token 驱动；操作面板宽度 token 在单列吸顶条下不再被消费。
  assert.match(css, /repeat\(auto-fill,\s*minmax\(var\(--layout-mod-card-min-width\),\s*1fr\)\)/);
  assert.match(css, /\.mod-card__poster[\s\S]*?height:\s*var\(--layout-mod-card-poster-height\);/);
  assert.doesNotMatch(css, /\.mod-library__body[\s\S]*?minmax\(0,\s*1fr\)\s+168px;/);
  assert.doesNotMatch(css, /\.mod-grid[\s\S]*?minmax\(200px,\s*1fr\)/);
  assert.doesNotMatch(css, /\.mod-card__poster[\s\S]*?height:\s*268px;/);
});

// ===== L2: 小屏契约负向保护（不得删除/破坏）=====

test("AppFrame 小屏契约保留：1360px 状态栏降级 + 860px shell 单列", () => {
  const rules = parseCssRules(readProjectFile("src/app/frame/AppFrame.css"));

  expectDeclaration(
    findRule(rules, ".window-tools", "(max-width: 1360px)"),
    /display:\s*none;/,
    "缺少 1360px 下隐藏 window tools 的规则",
  );
  expectDeclaration(
    findRule(rules, ".status-pill:not(.compact)", "(max-width: 1360px)"),
    /display:\s*none;/,
    "缺少 1360px 下隐藏非 compact status pill 的规则",
  );
  expectDeclaration(
    findRule(rules, ".app-shell:not([data-sidebar-mode=\"floating\"])", "(max-width: 860px)"),
    /grid-template-columns:\s*1fr;/,
    "缺少 860px 下 shell 单列规则",
  );
  expectDeclaration(
    findRule(rules, ".app-surface", "(max-width: 860px)"),
    /padding:\s*16px;/,
    "缺少 860px 下 surface padding 收缩规则",
  );
});

test("RouterOutlet 与 Dashboard 小屏契约保留：1360px 单列化", () => {
  for (const file of ["src/app/routing/RouterOutlet.css", "src/features/dashboard/Dashboard.css"]) {
    const rules = parseCssRules(readProjectFile(file));
    const selector = file.includes("RouterOutlet") ? ".route-transition__layer" : ".workbench-body";

    expectDeclaration(
      findRule(rules, selector, "(max-width: 1360px)"),
      /grid-template-columns:\s*1fr;/,
      `${file} 缺少 1360px 单列化`,
    );
  }
});

test("关键承压容器保留 min-width: 0 护栏", () => {
  const checks = [
    ["src/app/frame/AppFrame.css", [".app-surface", ".top-status-bar", ".current-game"]],
    ["src/app/routing/RouterOutlet.css", [".route-transition", ".route-transition__layer"]],
    ["src/features/dashboard/Dashboard.css", [".workbench-body", ".main-workspace", ".setup-rail"]],
    [
      "src/features/mods/ModLibraryPage.css",
      [
        [".mod-library", null],
        [".mod-library__toolbar-slot,\n.mod-library__actions-slot", null],
        [".mod-library__content", null],
        [".mod-library__sticky-controls", null],
        [".compact-panel", null],
        [".compact-panel__stack", null],
        [".compact-action__left", null],
      ],
    ],
  ];

  for (const [file, selectors] of checks) {
    const rules = parseCssRules(readProjectFile(file));

    for (const entry of selectors) {
      const [selector, media] = Array.isArray(entry) ? entry : [entry, null];
      const scope = media ? `${selector} @ ${media}` : selector;

      expectDeclaration(
        findRule(rules, selector, media),
        /min-width:\s*0;/,
        `${file} 缺少关键容器护栏: ${scope}`,
      );
    }
  }
});

test("Mod 管理页小屏契约保留：960/640 断点", () => {
  const rules = parseCssRules(readProjectFile("src/features/mods/ModLibraryPage.css"));

  expectDeclaration(
    findRule(rules, ".mod-library", "(max-width: 960px)"),
    /--layout-mod-card-min-width:\s*170px;/,
    "缺少 960px 下 mod 卡片最小宽度 token 覆盖",
  );
  expectDeclaration(
    findRule(rules, ".mod-library", "(max-width: 640px)"),
    /--layout-mod-card-min-width:\s*150px;/,
    "缺少 640px 下 mod 卡片最小宽度 token 覆盖",
  );
  expectDeclaration(
    findRule(rules, ".mod-library", "(max-width: 640px)"),
    /--layout-mod-card-poster-height:\s*220px;/,
    "缺少 640px 下海报高度 token 覆盖",
  );
});

test("悬浮侧边栏留白跟随页面内边距，保证与顶部状态栏顶边对齐", () => {
  const rules = parseCssRules(readProjectFile("src/app/shell/layouts/floating-sidebar/FloatingSidebar.css"));
  const base = findRule(rules, ".floating-sidebar");
  const narrow = findRule(rules, ".floating-sidebar", "(max-width: 860px)");

  // 状态栏顶边由 .app-surface 的 padding 决定，该 padding 是随分辨率变化的 token。
  // 侧边栏若写死 28px，在 --layout-page-padding 升到 32px / 36px 的 2K/4K 下会高出 4-8px。
  expectDeclaration(
    base,
    /height:\s*calc\(100vh - var\(--layout-page-padding\) \* 2\);/,
    "悬浮侧边栏上下留白必须跟随 --layout-page-padding",
  );
  expectDeclaration(
    base,
    /margin-left:\s*var\(--layout-page-padding\);/,
    "悬浮侧边栏左侧留白必须跟随 --layout-page-padding",
  );
  // ≤860px 时 .app-surface 内边距收缩到 16px，侧边栏必须同步收缩。
  expectDeclaration(narrow, /height:\s*calc\(100vh - 32px\);/, "860px 下侧边栏留白未与 16px 内边距对齐");
  expectDeclaration(narrow, /margin-left:\s*16px;/, "860px 下侧边栏左留白未与 16px 内边距对齐");

  // .floating-sidebar 没有 position，top/bottom/left 属于不生效的死声明，不得再出现。
  for (const rule of rules.filter((entry) => entry.selector === ".floating-sidebar")) {
    assert.doesNotMatch(
      rule.body,
      /(^|[;{])\s*(top|bottom|left|right):/,
      `.floating-sidebar 未定位，偏移声明不会生效: ${rule.media ?? "base"}`,
    );
  }
});

// ===== L2: 断点方向不冲突 =====

test("宽屏断点全部为 min-width，不与 max-width 小屏断点方向冲突", () => {
  const tokensCss = readProjectFile("src/shared/styles/tokens.css");
  const wideBlocks = tokensCss.match(/@media\s*\(min-width:[^)]+\)\s*{[\s\S]*?--layout-/g) ?? [];

  assert.ok(wideBlocks.length >= 3, "宽屏断点至少应有 3 个（1921/2561/3201）");

  for (const block of wideBlocks) {
    assert.match(block, /min-width:\s*(1921|2561|3201)px/, `意外断点值: ${block}`);
  }
});
