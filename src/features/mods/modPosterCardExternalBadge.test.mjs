// #286 3b-2 的源码形状门禁：卡片消费徽标 + 「弹窗 → 页 → 卡片」的会话级提升链。
// 纯投影行为在 externalCardBadge.test.mjs；这里只锁接线不被静默拆掉。

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const currentDirectory = dirname(fileURLToPath(import.meta.url));
const readSource = (name) => readFileSync(join(currentDirectory, name), "utf8");

const cardSource = readSource("ModPosterCard.tsx");
const cardStyles = readSource("ModPosterCard.css");
const pageSource = readSource("ModLibraryPage.tsx");
const dialogSource = readSource("ModDetailDialog.tsx");
const sectionSource = readSource("ExternalStateSection.tsx");
const hookSource = readSource("useExternalModState.ts");

test("卡片状态位消费徽标：pill 与 tech 行都以徽标文案取代状态文案", () => {
  assert.match(cardSource, /projectExternalCardBadge/);
  // pill：徽标存在时取代状态文案位，statusLabelForItem 是无结果时的回退。
  assert.match(
    cardSource,
    /\{externalBadge \? externalBadge\.text : statusLabelForItem\(item, card\)\}/,
  );
  // tech 行：完整档文案取代 READY/ACTIVE 状态词。
  assert.match(
    cardSource,
    /\{externalBadge \? externalBadge\.text : techStatusLabel\[item\.status\]\}/,
  );
  // 完整事实经 title/aria 暴露，语义 case 经 data 属性供配色。
  assert.match(cardSource, /data-external-case=\{externalBadge\?\.case\}/);
  assert.match(cardSource, /title=\{externalBadge\?\.label\}/);
  assert.match(cardSource, /aria-label=\{externalBadge\?\.label\}/);
  // 「需留意」的判定必须换警示图标，不能顶着对勾说「已被改动」。
  assert.match(cardSource, /externalBadgeAlerts \?[\s\S]*?TriangleAlert/);
});

test("9c 全占用改口：卡片换锁图标而非对勾/警示，pill 与 tech 行都有 occupied 配色", () => {
  // 图标：occupied 优先于「需留意」判定；对勾会读成「已安装」，警示又暗示有东西坏了。
  assert.match(cardSource, /const externalBadgeOccupied = externalBadge\?\.case === "occupied";/);
  assert.match(cardSource, /externalBadgeOccupied \?[\s\S]*?<Lock /);
  assert.match(cardSource, /!externalBadgeOccupied &&/);
  // 配色：不能落回 is-not_installed 的灰或 installed 的绿，两处状态位都要有专属规则。
  assert.match(cardStyles, /\.mod-card__status-pill\[data-external-case="occupied"\] \{/);
  assert.match(
    cardStyles,
    /\.mod-grid\.view-tech \.mod-card__tech-status\[data-external-case="occupied"\] \{/,
  );
});

test("提升链完整：hook 上报 → section 透传 → 弹窗透传 → 页级 Map → 卡片", () => {
  // hook：每个 getter 结果都上报（含世代漂移时——事实仍有效，只是本地不更新）。
  assert.match(hookSource, /onResultRef\.current\?\.\(requestModId, dto\)/);
  // section 把 onResult 交给 hook。
  assert.match(sectionSource, /useExternalModState\(\{[^}]*onResult[^}]*\}\)/);
  // 弹窗把页级回调接到 section。
  assert.match(dialogSource, /onResult=\{onExternalStateResult\}/);
  // 页面：回调进弹窗，Map 里的结果按 item.id 发给卡片。
  assert.match(pageSource, /onExternalStateResult=\{recordExternalStateResult\}/);
  assert.match(pageSource, /externalState=\{externalStateResults\.get\(item\.id\) \?\? null\}/);
});

test("会话边界（A+）：结果表挂在应用级 Provider、按 (game, profile) 作用域读写，页面不再自持 Map", () => {
  const appSource = readFileSync(join(currentDirectory, "../../App.tsx"), "utf8");
  const providerSource = readSource("ExternalStateSessionProvider.tsx");

  // 页面：不再有页级 Map；结果与上报都来自会话表 hook，作用域就是 profileContext（无配置档时为 null）。
  assert.doesNotMatch(pageSource, /setExternalStateResults|useState<\s*ReadonlyMap<string, ExternalModStateDto>/);
  assert.match(
    pageSource,
    /const \{ results: externalStateResults, record: recordExternalStateResult \} =\s*useExternalStateSession\(profileContext\)/,
  );
  // Provider 必须在 RouterOutlet 之上：路由切换卸载页面，表要活过切页。
  const providerOpen = appSource.indexOf("<ExternalStateSessionProvider>");
  const outlet = appSource.indexOf("<RouterOutlet />");
  const providerClose = appSource.indexOf("</ExternalStateSessionProvider>");
  assert.ok(providerOpen !== -1 && outlet !== -1 && providerClose !== -1);
  assert.ok(providerOpen < outlet && outlet < providerClose, "ExternalStateSessionProvider 必须包住 RouterOutlet");
  // Provider 只持有与派发，语义走纯逻辑模块：读按作用域隔离、写按作用域换新。
  assert.match(providerSource, /externalStateResultsForScope\(/);
  assert.match(providerSource, /recordExternalStateResult\(previous, currentScope, modId, state\)/);
  assert.match(providerSource, /if \(currentScope === null\) \{\s*return;/);
});
