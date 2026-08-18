import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import ts from "typescript";

const repoRoot = process.cwd();

async function importTypeScriptModule(relativePath) {
  const source = readFileSync(join(repoRoot, relativePath), "utf8");
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      target: ts.ScriptTarget.ES2022,
    },
    fileName: relativePath,
  });
  const dataUrl = `data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`;
  return import(dataUrl);
}

test("tour geometry expands, clamps, and chooses responsive panel docking", async () => {
  const { expandAndClampRect, rectsEqual, shouldDockTourPanel } = await importTypeScriptModule(
    "src/shared/onboarding/tourGeometry.ts",
  );

  assert.deepEqual(
    expandAndClampRect(
      { top: 4, right: 98, bottom: 76, left: 3 },
      8,
      100,
      80,
    ),
    { top: 0, right: 100, bottom: 80, left: 0, width: 100, height: 80 },
  );
  assert.deepEqual(
    expandAndClampRect(
      { top: 20, right: 60, bottom: 50, left: 20 },
      6,
      100,
      80,
    ),
    { top: 14, right: 66, bottom: 56, left: 14, width: 52, height: 42 },
  );
  assert.equal(
    rectsEqual(
      { top: 14, right: 66, bottom: 56, left: 14, width: 52, height: 42 },
      { top: 14.2, right: 66.2, bottom: 55.8, left: 13.8, width: 52.4, height: 42 },
    ),
    true,
  );
  assert.equal(shouldDockTourPanel(null, 600, 800), true);
  assert.equal(shouldDockTourPanel(null, 1280, 800), false);
  assert.equal(
    shouldDockTourPanel(
      { top: 20, right: 1050, bottom: 700, left: 20, width: 1030, height: 680 },
      1280,
      800,
    ),
    true,
  );
  assert.equal(
    shouldDockTourPanel(
      { top: 20, right: 420, bottom: 280, left: 20, width: 400, height: 260 },
      1280,
      800,
    ),
    false,
  );
});

test("tour storage fails open for missing or corrupt state and respects content versions", async () => {
  const {
    ONBOARDING_STORAGE_KEY,
    readOnboardingState,
    saveTourOutcome,
    shouldAutoStartTour,
  } = await importTypeScriptModule("src/shared/onboarding/tourStorage.ts");
  const definition = { id: "hmm.first-run", contentVersion: 3, steps: [] };
  const values = new Map();
  const storage = {
    getItem: (key) => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  };

  assert.equal(shouldAutoStartTour(definition, null), true);
  values.set(ONBOARDING_STORAGE_KEY, "not-json");
  assert.deepEqual(readOnboardingState(storage), { schemaVersion: 1, tours: {} });
  assert.equal(shouldAutoStartTour(definition, storage), true);

  assert.equal(saveTourOutcome(definition, "skipped", storage), true);
  assert.equal(shouldAutoStartTour(definition, storage), false);
  assert.equal(readOnboardingState(storage).tours[definition.id].outcome, "skipped");

  values.set(ONBOARDING_STORAGE_KEY, JSON.stringify({
    schemaVersion: 1,
    tours: {
      [definition.id]: { contentVersion: 1, outcome: "completed" },
    },
  }));
  assert.equal(shouldAutoStartTour(definition, storage), true);
});

test("tour target candidates prefer the precise anchor and de-duplicate fallbacks", async () => {
  const { getTourAnchorCandidates } = await importTypeScriptModule(
    "src/shared/onboarding/tourTarget.ts",
  );

  assert.deepEqual(getTourAnchorCandidates("profiles.backup-history", "profiles.settings"), [
    "profiles.backup-history",
    "profiles.settings",
  ]);
  assert.deepEqual(getTourAnchorCandidates("profiles.settings", "profiles.settings"), [
    "profiles.settings",
  ]);
  assert.deepEqual(getTourAnchorCandidates(undefined, "page.profiles"), ["page.profiles"]);
});

test("tour targets reject disabled ancestors instead of highlighting unusable controls", () => {
  const source = readFileSync("src/shared/onboarding/tourTarget.ts", "utf8");

  assert.match(
    source,
    /element\.closest\('\[aria-disabled="true"\], fieldset:disabled'\)/,
  );
  assert.match(source, /if \(element\.hidden\) return false;/);
});
