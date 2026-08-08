import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";

const repoRoot = process.cwd();
const windowsConfigPath = path.join(
  repoRoot,
  "src-tauri",
  "tauri.windows.conf.json",
);
const hookPath = path.join(
  repoRoot,
  "src-tauri",
  "windows",
  "nsis-installer-hooks.nsh",
);

function readJson(filePath) {
  return JSON.parse(readFileSync(filePath, "utf8"));
}

test("points the Windows bundle at the controlled NSIS hook", () => {
  const config = readJson(windowsConfigPath);
  assert.equal(
    config.bundle.windows.nsis.installerHooks,
    "windows/nsis-installer-hooks.nsh",
  );
  assert.deepEqual(config.bundle.externalBin, [
    "binaries/hmm-save-backup-worker",
    "binaries/hmm-save-backup-installer-cleanup",
  ]);
});

test("NSIS hook owns only the pre-uninstall macro", () => {
  const hook = readFileSync(hookPath, "utf8");
  assert.match(hook, /^!macro NSIS_HOOK_PREUNINSTALL\r?$/m);
  assert.match(hook, /^!macroend\r?$/m);
  assert.doesNotMatch(hook, /NSIS_HOOK_PREINSTALL|NSIS_HOOK_POST/);
  assert.doesNotMatch(hook, /schtasks|Stop-ScheduledTask|PowerShell|<Task/i);
});

test("runs the fixed helper without installer-controlled arguments", () => {
  const hook = readFileSync(hookPath, "utf8");
  assert.match(
    hook,
    /ExecWait\s+'"\$INSTDIR\\hmm-save-backup-installer-cleanup\.exe"'\s+\$0/,
  );
  assert.doesNotMatch(hook, /task.?name|SID|owner.?marker|XML/i);
  assert.doesNotMatch(hook, /\$CMDLINE|\$R[0-9]+\s+.*(?:task|path)/i);
});

test("skips upgrade cleanup and fails closed for every nonzero helper code", () => {
  const hook = readFileSync(hookPath, "utf8");
  assert.match(hook, /\$UpdateMode\s*=\s*1/);
  for (const exitCode of [20, 21, 22, 23, 64]) {
    assert.match(hook, new RegExp(`\\$0\\s*=\\s*${exitCode}`));
  }
  assert.match(hook, /\$0\s*=\s*0/);
  assert.match(hook, /\$\{Silent\}/);
  assert.match(hook, /SetErrorLevel\s+\$0/);
  assert.match(hook, /Quit/);
  assert.match(hook, /MessageBox/);
});
