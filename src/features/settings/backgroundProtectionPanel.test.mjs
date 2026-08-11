import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

const PANEL_PATH = "src/features/settings/BackgroundProtectionPanel.tsx";
const SETTINGS_PAGE_PATH = "src/features/settings/SettingsPage.tsx";
const SETTINGS_CSS_PATH = "src/features/settings/SettingsPage.css";

function readProjectFile(path) {
  return readFileSync(path, "utf8");
}

test("background protection panel exposes accessible guarded controls", () => {
  assert.equal(existsSync(PANEL_PATH), true, "background protection panel should exist");

  const source = readProjectFile(PANEL_PATH);

  assert.match(source, /role="status"/);
  assert.match(source, /aria-live="polite"/);
  assert.match(source, /aria-atomic="true"/);
  assert.match(source, /status === "starting"/);
  assert.match(source, /status === "unsupported_platform"/);
  assert.match(source, /onChange/);
  assert.match(source, /disabled=\{busy/);
  assert.match(source, /peekBackgroundProtectionControlStatus/);
  assert.match(source, /retainedPanelState/);
  assert.match(source, /retainedPanelState\.control !== cachedControl/);
  assert.match(source, /getBackgroundProtectionControlStatus\(\{ force: true \}\)/);
  assert.match(source, /background-protection-panel__switch-control/);
  assert.match(source, /正在启用后台保护/);
  assert.match(source, /后台保护已启用/);
  assert.match(source, /STARTING_AUTO_REFRESH_DELAYS_MS/);
  assert.match(source, /source === "automatic" && autoVerificationArmedRef\.current/);
  assert.match(source, /performance\.now\(\)/);
  assert.match(source, /formatBackgroundProtectionDuration/);
  assert.match(source, /hasBackgroundProtectionConverged/);
  assert.match(source, /系统状态已自动重新同步/);
  assert.match(source, /HMM 会在当前页面自动复查/);
  assert.doesNotMatch(source, /<label className="setting-row background-protection-panel__toggle">/);
  assert.match(source, /getBackgroundProtectionControlStatus\(/);
  assert.match(source, /enableBackgroundProtection/);
  assert.match(source, /disableBackgroundProtection/);
  assert.match(source, /重试启用/);
  assert.match(source, /重试停用/);
  assert.match(source, /停用保护/);
  assert.match(source, /const switchChecked = unsupported/);
  assert.match(source, /checked=\{switchChecked\}/);
  assert.match(source, /changeProtection\(state\.control\.desiredEnabled\)/);
  assert.doesNotMatch(source, /请点击“重新检查”确认是否已保护/);
  assert.doesNotMatch(source, /完成后会自动变为已保护/);
  assert.doesNotMatch(source, /error\.message|taskName|taskXml|workerPath|PowerShell|sid|leaseOwner/i);
});

test("background protection status helper maps every stable status", async () => {
  const module = await import("./backgroundProtectionTypes.ts");
  assert.equal(typeof module.getBackgroundProtectionCopy, "function");

  const cases = [
    ["not_enabled", "未启用", "neutral", "none"],
    ["starting", "正在验证后台保护", "warning", "none"],
    ["protected", "已保护", "success", "none"],
    ["registration_failed", "注册未完成", "danger", "retry"],
    ["worker_unhealthy", "后台运行异常", "danger", "retry"],
    ["permission_required", "需要系统权限", "warning", "retry"],
    ["unsupported_platform", "当前平台不支持", "neutral", "none"],
  ];

  for (const [status, label, tone, action] of cases) {
    const copy = module.getBackgroundProtectionCopy(status);
    assert.equal(copy.label, label);
    assert.equal(copy.tone, tone);
    assert.equal(copy.action, action);
    assert.equal(typeof copy.description, "string");
    assert.notEqual(copy.description.length, 0);
  }

  assert.doesNotMatch(module.getBackgroundProtectionCopy("starting").description, /已保护/);
  assert.deepEqual(module.getBackgroundProtectionCopy("future_status"), {
    label: "状态不可用",
    description: "无法识别后台保护状态，请重新检查。",
    tone: "danger",
    action: "retry",
  });
});

test("background protection errors map to fixed local copy", async () => {
  const module = await import("./backgroundProtectionTypes.ts");
  assert.equal(typeof module.getBackgroundProtectionErrorCode, "function");
  assert.equal(typeof module.getBackgroundProtectionErrorMessage, "function");

  assert.equal(
    module.getBackgroundProtectionErrorCode({
      code: "save_backup_background_settings_unavailable",
      message: "C:/Users/Alice/save",
    }),
    "save_backup_background_settings_unavailable",
  );
  assert.equal(module.getBackgroundProtectionErrorCode(new Error("C:/Users/Alice/save")), "unknown");

  const known = module.getBackgroundProtectionErrorMessage("save_backup_background_permission_required");
  assert.equal(known, "系统拒绝更新后台任务，请检查当前账户权限后重试。");

  const unknown = module.getBackgroundProtectionErrorMessage("C:/Users/Alice/save");
  assert.equal(unknown, "后台保护操作未完成，请重新检查状态后重试。");
  assert.doesNotMatch(unknown, /C:\/Users|Alice|save/);
});

test("background protection operation helpers keep convergence and timing explicit", async () => {
  const module = await import("./backgroundProtectionTypes.ts");

  assert.equal(
    module.hasBackgroundProtectionConverged(
      {
        desiredEnabled: true,
        status: "starting",
        enabledAt: 1,
        lastHeartbeatAt: null,
        lastErrorCode: null,
      },
      true,
    ),
    true,
  );
  assert.equal(
    module.hasBackgroundProtectionConverged(
      {
        desiredEnabled: true,
        status: "protected",
        enabledAt: 1,
        lastHeartbeatAt: 2,
        lastErrorCode: null,
      },
      true,
    ),
    true,
  );
  assert.equal(
    module.hasBackgroundProtectionConverged(
      {
        desiredEnabled: true,
        status: "registration_failed",
        enabledAt: 1,
        lastHeartbeatAt: null,
        lastErrorCode: "save_backup_background_registration_failed",
      },
      true,
    ),
    false,
  );
  assert.equal(
    module.hasBackgroundProtectionConverged(
      {
        desiredEnabled: false,
        status: "not_enabled",
        enabledAt: null,
        lastHeartbeatAt: null,
        lastErrorCode: null,
      },
      false,
    ),
    true,
  );
  assert.equal(module.formatBackgroundProtectionDuration(0), "不足 0.1 秒");
  assert.equal(module.formatBackgroundProtectionDuration(Number.NaN), "不足 0.1 秒");
  assert.equal(module.formatBackgroundProtectionDuration(1_234), "1.2 秒");
});

test("settings hosts the persisted panel outside session preview state", () => {
  const page = readProjectFile(SETTINGS_PAGE_PATH);
  const css = readProjectFile(SETTINGS_CSS_PATH);

  assert.match(page, /import \{ BackgroundProtectionPanel \} from "\.\/BackgroundProtectionPanel"/);
  assert.match(
    page,
    /后台保护与窗口关闭偏好会正式保存；其余标记为预览的选项只在当前会话中生效。/,
  );
  assert.doesNotMatch(page, /当前真正后台守护尚未落地/);

  const backupSectionIndex = page.indexOf('title="存档备份"');
  const panelIndex = page.indexOf("<BackgroundProtectionPanel />");
  const previewReminderIndex = page.indexOf('title="安装前提醒备份"');
  assert.ok(backupSectionIndex >= 0);
  assert.ok(panelIndex > backupSectionIndex);
  assert.ok(previewReminderIndex > panelIndex);
  assert.doesNotMatch(page.slice(0, page.indexOf("export function SettingsPage")), /BackgroundProtectionControlDto/);

  assert.match(css, /\.background-protection-panel\s*\{[\s\S]*?min-height:/);
  assert.match(css, /\.background-protection-panel__switch-control\s*\{/);
  assert.match(css, /\.background-protection-panel__toggle:focus-within\s*\{[\s\S]*?outline:\s*none;/);
  assert.match(css, /\.background-protection-panel__switch-control:hover input:checked \+ \.setting-switch/);
  assert.match(css, /\.background-protection-panel__operation\.is-visible/);
  assert.match(css, /\.background-protection-panel__operation\.is-busy::after/);
  assert.match(css, /@keyframes background-protection-progress/);
  assert.match(css, /\.background-protection-panel__timer/);
  assert.match(css, /\.background-protection-panel__action:focus-visible/);
  assert.match(css, /@media \(max-width: 600px\)[\s\S]*?\.background-protection-panel__summary/);
  assert.match(
    css,
    /@media \(prefers-reduced-motion: reduce\)[\s\S]*?\.background-protection-spinner\s*\{[\s\S]*?animation:\s*none !important;/,
  );
});
