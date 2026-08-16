import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readProjectFile(path) {
  return readFileSync(path, "utf8");
}

function getRuleBody(css, selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = css.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`));
  assert.ok(match, `missing CSS rule: ${selector}`);
  return match[1];
}

test("active route layers do not trap page overlays beneath the app header", () => {
  const routerCss = readProjectFile("src/app/routing/RouterOutlet.css");
  const frameCss = readProjectFile("src/app/frame/AppFrame.css");
  const activeLayer = getRuleBody(routerCss, ".route-transition__layer.is-active");
  const exitingLayer = getRuleBody(routerCss, ".route-transition__layer.is-exiting");
  const headerDock = getRuleBody(frameCss, ".app-surface__header-dock");

  // A numeric z-index on the active grid item creates a stacking context. Any
  // fixed dialog rendered by the route is then unable to rise above the app
  // header, regardless of the dialog's own z-index.
  assert.match(activeLayer, /z-index:\s*auto;/);
  assert.doesNotMatch(activeLayer, /z-index:\s*0;/);

  // The exiting layer still needs explicit ordering while two routes overlap.
  assert.match(exitingLayer, /z-index:\s*1;/);
  assert.match(headerDock, /z-index:\s*40;/);
});
