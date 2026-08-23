import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

const read = (path) => readFileSync(path, "utf8");

test("about page is an enabled route with fixed project links", () => {
  const routeTypes = read("src/app/routing/routeTypes.ts");
  const routes = read("src/app/routing/routeRegistry.tsx");
  const nav = read("src/app/shell/navigation/navItems.ts");
  const page = read("src/features/about/AboutPage.tsx");
  const classicSidebar = read("src/app/shell/layouts/classic-sidebar/ClassicSidebar.tsx");
  const floatingSidebar = read("src/app/shell/layouts/floating-sidebar/FloatingSidebar.tsx");
  const floatingSidebarCss = read("src/app/shell/layouts/floating-sidebar/FloatingSidebar.css");

  assert.equal(existsSync("src/features/about/AboutPage.css"), true);
  assert.match(routeTypes, /"about"/);
  assert.match(routes, /id:\s*"about"[\s\S]*?path:\s*"\/about"[\s\S]*?element:\s*AboutPage/);
  assert.match(nav, /id:\s*"about"[\s\S]*?placement:\s*"utility"/);
  assert.match(read("src/app/appShellCopy.ts"), /about: "关于"/);
  assert.match(page, /getVersion\(\)/);
  assert.match(page, /packageMetadata\.version/);
  assert.match(page, /openUrl\(ABOUT_LINK_HREFS\[linkId\]\)/);
  // I18N-01 起页面文案收敛到 aboutPageCopy；"未启用自动更新"的事实钉在 zh_cn 字典，
  // 页面只允许经 copy.release.description 渲染。
  const aboutCopy = read("src/features/about/aboutPageCopy.ts");
  assert.match(aboutCopy, /当前尚未启用应用内自动更新/);
  assert.match(page, /\{copy\.release\.description\}/);
  assert.match(page, /https:\/\/github\.com\/TheLostRiver\/HelsincyModManager\/releases/);
  assert.match(page, /https:\/\/github\.com\/TheLostRiver\/HelsincyModManager\/issues/);
  assert.match(page, /data-tour-id="about\.release"/);
  assert.match(page, /data-tour-id="about\.links"/);
  assert.match(classicSidebar, /className="sidebar-utility-nav"/);
  assert.match(floatingSidebar, /className="floating-sidebar__utility-nav"/);
  assert.match(floatingSidebarCss, /@media \(max-height: 700px\) and \(min-width: 861px\)/);
});

test("desktop external links use a constrained opener capability", () => {
  const tauri = read("src-tauri/src/lib.rs");
  const capability = read("src-tauri/capabilities/default.json");
  const packageJson = read("package.json");
  const cargo = read("src-tauri/Cargo.toml");

  assert.match(tauri, /tauri_plugin_opener::init\(\)/);
  assert.match(packageJson, /"@tauri-apps\/plugin-opener"/);
  assert.match(cargo, /tauri-plugin-opener/);
  assert.match(capability, /"identifier":\s*"opener:allow-open-url"/);
  assert.match(capability, /https:\/\/github\.com\/TheLostRiver"/);
  assert.match(capability, /https:\/\/github\.com\/TheLostRiver\/\*\*/);
  assert.doesNotMatch(capability, /https:\/\/\*\*/);
});

test("sponsor information has one maintained documentation source", () => {
  const sponsor = read("docs/SPONSOR.md");
  const readme = read("README.md");

  assert.match(sponsor, /https:\/\/afdian\.com\/a\/Helsincy/);
  assert.match(sponsor, /https:\/\/ko-fi\.com\/helsincy/);
  assert.match(sponsor, /assets\/support\/wechat-reward-code\.jpg/);
  assert.match(readme, /\[赞助与支持\]\(docs\/SPONSOR\.md\)/);
  assert.doesNotMatch(readme, /https:\/\/afdian\.com|https:\/\/ko-fi\.com/);
});
