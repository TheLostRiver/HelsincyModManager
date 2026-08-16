import assert from "node:assert/strict";
import { readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { cwd } from "node:process";
import { test } from "node:test";

const repoRoot = cwd();
const logoPath = join(repoRoot, "public", "branding", "hmm-logo.png");

function readProjectFile(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

test("app shell shares one decorative brand mark", () => {
  const brandMark = readProjectFile("src/app/branding/AppBrandMark.tsx");
  const classicSidebar = readProjectFile(
    "src/app/shell/layouts/classic-sidebar/ClassicSidebar.tsx",
  );
  const floatingSidebar = readProjectFile(
    "src/app/shell/layouts/floating-sidebar/FloatingSidebar.tsx",
  );

  assert.match(brandMark, /APP_BRAND_LOGO_SRC\s*=\s*"\/branding\/hmm-logo\.png"/);
  assert.match(brandMark, /alt=""/);
  assert.match(brandMark, /aria-hidden="true"/);
  assert.match(classicSidebar, /<AppBrandMark className="brand-block__mark" \/>/);
  assert.match(floatingSidebar, /<AppBrandMark className="floating-sidebar__brand-mark" \/>/);
  assert.doesNotMatch(floatingSidebar, />\s*H\s*</);
});

test("browser favicon and bundled brand mark use the optimized PNG", () => {
  const indexHtml = readProjectFile("index.html");
  const logoBytes = readFileSync(logoPath);
  const pngSignature = [137, 80, 78, 71, 13, 10, 26, 10];

  assert.match(
    indexHtml,
    /<link rel="icon" type="image\/png" href="\/branding\/hmm-logo\.png" \/>/,
  );
  assert.deepEqual([...logoBytes.subarray(0, 8)], pngSignature);
  assert.ok(statSync(logoPath).size <= 128 * 1024, "sidebar logo should stay below 128 KiB");
});

test("both sidebar layouts keep the brand mark contained", () => {
  for (const relativePath of [
    "src/app/shell/layouts/classic-sidebar/ClassicSidebar.css",
    "src/app/shell/layouts/floating-sidebar/FloatingSidebar.css",
  ]) {
    const css = readProjectFile(relativePath);
    assert.match(css, /object-fit:\s*contain;/, `${relativePath} must contain the logo`);
  }
});

test("Tauri rebuilds native resources when desktop icons change", () => {
  const buildScript = readProjectFile("src-tauri/build.rs");
  const tauriConfig = readProjectFile("src-tauri/tauri.conf.json");

  assert.match(buildScript, /cargo:rerun-if-changed=icons\/icon\.ico/);
  assert.match(buildScript, /cargo:rerun-if-changed=icons\/icon\.png/);
  assert.match(tauriConfig, /"icons\/icon\.ico"/);
  assert.match(tauriConfig, /"icons\/icon\.png"/);
});
