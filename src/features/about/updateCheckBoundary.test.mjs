import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

// 「检查更新」的**网络边界**守卫。
//
// 这条对应验收标准第 6 条：网络访问仍在 CSP / Tauri capability 白名单内，未放宽。
// 它是本功能最容易在后续迭代中被悄悄破坏的一条——只要有人图省事把请求挪到前端，
// 就得给 CSP 的 connect-src 加上 api.github.com，而那等于**放宽整个前端的网络策略**：
// 之后任何前端代码都能往 GitHub 发请求。
//
// 因此把「网络请求必须留在 Rust 侧」钉成测试，而不是只写在文档里。

const read = (path) => readFileSync(path, "utf8");
const tauriConfig = read("src-tauri/tauri.conf.json");
const capabilities = read("src-tauri/capabilities/default.json");
const updateApi = read("src/features/about/updateCheckApi.ts");
const aboutPage = read("src/features/about/AboutPage.tsx");
const infraClient = read("src-tauri/crates/hmm-infra/src/release_update.rs");

test("the content security policy is not widened for the update check", () => {
  // CSP 里一旦出现 github，整个前端就都能往外发请求了。
  assert.doesNotMatch(
    tauriConfig,
    /github\.com/,
    "CSP 不得为更新检查放开 github.com —— 网络请求留在 Rust 侧就不需要放宽它",
  );
});

test("no capability grants the frontend a github network permission", () => {
  // opener 只允许打开 github.com/TheLostRiver** 的**页面**（用户主动点击）；
  // 若这里出现 api.github.com，说明有人给了前端发请求的能力。
  assert.doesNotMatch(
    capabilities,
    /api\.github\.com/,
    "capability 不得授予前端访问 api.github.com 的权限",
  );
});

test("the frontend never issues the release request itself", () => {
  for (const [name, source] of [
    ["updateCheckApi.ts", updateApi],
    ["AboutPage.tsx", aboutPage],
  ]) {
    assert.doesNotMatch(source, /\bfetch\s*\(/, `${name} 不得直接发起网络请求`);
    assert.doesNotMatch(source, /XMLHttpRequest/, `${name} 不得使用 XMLHttpRequest`);
    assert.doesNotMatch(source, /api\.github\.com/, `${name} 不得出现 GitHub API 地址`);
  }
});

test("the release url stays a compile-time constant in rust", () => {
  // URL 必须是编译期常量、不接受调用方输入，否则就存在把请求导向任意地址的可能。
  assert.match(infraClient, /const RELEASE_FEED_URL: &str =/);
  assert.doesNotMatch(
    infraClient,
    /fn (get_release_feed_json|latest_release_version)[^)]*url/,
    "transport 与 source 都不接受外部传入的 URL",
  );
});
