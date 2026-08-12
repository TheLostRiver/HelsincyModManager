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
const wixFragmentPath = path.join(
  repoRoot,
  "src-tauri",
  "windows",
  "wix",
  "installer-cleanup.wxs",
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
  assert.match(hook, /Abort/);
  assert.doesNotMatch(hook, /Quit/);
  assert.match(hook, /MessageBox/);
});

test("points WiX at the controlled cleanup fragment", () => {
  const config = readJson(windowsConfigPath);
  assert.equal(config.bundle.windows.wix.template, "windows/wix/main.wxs");
  assert.deepEqual(config.bundle.windows.wix.fragmentPaths, [
    "windows/wix/installer-cleanup.wxs",
  ]);
});

test("references the cleanup action from the locked WiX template", () => {
  const templatePath = path.join(
    repoRoot,
    "src-tauri",
    "windows",
    "wix",
    "main.wxs",
  );
  const template = readFileSync(templatePath, "utf8");
  assert.match(template, /<CustomActionRef Id="RunInstallerCleanup"\s*\/>/);
  assert.match(
    template,
    /<Error Id="1722">HMM could not complete a required setup action\. If you are uninstalling, close HMM and wait for any background backup to finish, then retry\. Otherwise, retry setup or collect the installer log\.<\/Error>/,
  );
  assert.doesNotMatch(
    template.match(/<Error Id="1722">([\s\S]*?)<\/Error>/)?.[1] ?? "",
    /task.?name|SID|owner.?marker|XML|PowerShell|worker.?path/i,
  );
});

test("runs the installed cleanup helper in user context before RemoveFiles", () => {
  const fragment = readFileSync(wixFragmentPath, "utf8");
  assert.match(
    fragment,
    /<CustomAction\s+Id="RunInstallerCleanup"\s+FileKey="Bin_hmm_save_backup_installer_cleanup\.exe"\s+ExeCommand=""\s+Execute="immediate"\s+Impersonate="yes"\s+Return="check"\s*\/>/,
  );
  assert.match(
    fragment,
    /<Custom\s+Action="RunInstallerCleanup"\s+Before="RemoveFiles">\s*REMOVE="ALL" AND NOT UPGRADINGPRODUCTCODE\s*<\/Custom>/,
  );
  assert.doesNotMatch(
    fragment,
    /schtasks|Stop-ScheduledTask|PowerShell|task.?name|SID|owner.?marker|<Task/i,
  );
});
