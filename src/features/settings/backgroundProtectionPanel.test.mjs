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
  // I18N-01 起文案收敛到 backgroundProtectionCopy：中文钉在 zh_cn 字典，面板只能经 copy 键渲染。
  const panelCopyModule = readProjectFile("src/features/settings/backgroundProtectionCopy.ts");
  assert.match(panelCopyModule, /正在启用后台保护/);
  assert.match(panelCopyModule, /后台保护已启用/);
  assert.match(source, /bpCopy\.panel\.busyEnable/);
  assert.match(source, /bpCopy\.toast\.enabledTitle/);
  assert.match(source, /BackgroundProtectionAutoVerificationScheduler/);
  assert.match(source, /automaticRefreshRef/);
  assert.match(source, /performance\.now\(\)/);
  assert.match(source, /formatBackgroundProtectionDuration/);
  assert.match(source, /hasBackgroundProtectionConverged/);
  assert.match(panelCopyModule, /系统状态已自动重新同步/);
  assert.match(source, /completed\.reconciled/);
  assert.match(source, /preserveBackgroundProtectionStateAfterRefreshFailure/);
  assert.match(source, /latestKnownPanelState/);
  assert.match(panelCopyModule, /本次检查未完成，当前仍显示最近一次成功确认的状态/);
  assert.match(source, /bpCopy\.panel\.refreshWarning/);
  assert.match(panelCopyModule, /HMM 正在自动复查/);
  assert.match(source, /bpCopy\.panel\.startingHintAuto/);
  assert.doesNotMatch(source, /<label className="setting-row background-protection-panel__toggle">/);
  assert.match(source, /getBackgroundProtectionControlStatus\(/);
  assert.match(source, /enableBackgroundProtection/);
  assert.match(source, /disableBackgroundProtection/);
  assert.match(panelCopyModule, /重试启用/);
  assert.match(panelCopyModule, /重试停用/);
  assert.match(panelCopyModule, /停用保护/);
  assert.match(source, /bpCopy\.panel\.retryEnable/);
  assert.match(source, /bpCopy\.panel\.stopProtection/);
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
    const copy = module.getBackgroundProtectionCopy(status, "zh_cn");
    assert.equal(copy.label, label);
    assert.equal(copy.tone, tone);
    assert.equal(copy.action, action);
    // 语义（tone/action）必须与语言无关：en 取词只换文本不换语义。
    const enCopy = module.getBackgroundProtectionCopy(status, "en");
    assert.equal(enCopy.tone, tone);
    assert.equal(enCopy.action, action);
    assert.notEqual(enCopy.label, label);
    assert.equal(typeof copy.description, "string");
    assert.notEqual(copy.description.length, 0);
  }

  assert.doesNotMatch(module.getBackgroundProtectionCopy("starting", "zh_cn").description, /已保护/);
  assert.deepEqual(module.getBackgroundProtectionCopy("future_status", "zh_cn"), {
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

  const known = module.getBackgroundProtectionErrorMessage(
    "save_backup_background_permission_required",
    "zh_cn",
  );
  assert.equal(known, "系统拒绝更新后台任务，请检查当前账户权限后重试。");

  // 脱敏语义与语言无关：任何 locale 下未知 code 都不得把 code 内容拼进消息。
  for (const locale of ["zh_cn", "en", "ja"]) {
    const unknown = module.getBackgroundProtectionErrorMessage("C:/Users/Alice/save", locale);
    assert.doesNotMatch(unknown, /C:\/Users|Alice|save/);
  }
  assert.equal(
    module.getBackgroundProtectionErrorMessage("C:/Users/Alice/save", "zh_cn"),
    "后台保护操作未完成，请重新检查状态后重试。",
  );
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
  assert.equal(module.formatBackgroundProtectionDuration(0, "zh_cn"), "不足 0.1 秒");
  assert.equal(module.formatBackgroundProtectionDuration(Number.NaN, "zh_cn"), "不足 0.1 秒");
  assert.equal(module.formatBackgroundProtectionDuration(1_234, "zh_cn"), "1.2 秒");
  assert.equal(module.formatBackgroundProtectionDuration(1_234, "en"), "1.2 s");
  assert.equal(module.formatBackgroundProtectionDuration(1_234, "ja"), "1.2 秒");
});

test("settings hosts the persisted panel outside session preview state", () => {
  const page = readProjectFile(SETTINGS_PAGE_PATH);
  const css = readProjectFile(SETTINGS_CSS_PATH);

  assert.match(page, /import \{ BackgroundProtectionPanel \} from "\.\/BackgroundProtectionPanel"/);
  // I18N-01 起设置页文案收敛到 settingsPageCopy："哪些设置会正式保存"的声明钉在 zh_cn 字典，
  // 页面只允许经 copy.hero.description 渲染，不得回退为硬编码字面量。
  const copyModule = readProjectFile("src/features/settings/settingsPageCopy.ts");
  assert.match(
    copyModule,
    /后台保护与窗口关闭偏好会正式保存；其余标记为预览的选项只在当前会话中生效。/,
  );
  assert.match(page, /\{copy\.hero\.description\}/);
  assert.doesNotMatch(page, /当前真正后台守护尚未落地/);
  assert.doesNotMatch(copyModule, /当前真正后台守护尚未落地/);

  const backupSectionIndex = page.indexOf("title={copy.saveBackup.title}");
  const panelIndex = page.indexOf("<BackgroundProtectionPanel />");
  const previewReminderIndex = page.indexOf("title={copy.saveBackup.backupReminder.title}");
  assert.ok(backupSectionIndex >= 0);
  assert.ok(panelIndex > backupSectionIndex);
  assert.ok(previewReminderIndex > panelIndex);
  assert.doesNotMatch(page, /自动备份节奏/);
  assert.doesNotMatch(page, /backupCadence/);
  assert.doesNotMatch(page, /TimePickerPopover/);
  assert.doesNotMatch(page.slice(0, page.indexOf("export function SettingsPage")), /BackgroundProtectionControlDto/);

  assert.match(css, /\.background-protection-panel\s*\{[\s\S]*?min-height:/);
  assert.match(css, /\.background-protection-panel__switch-control\s*\{/);
  assert.match(css, /\.background-protection-panel__toggle:focus-within\s*\{[\s\S]*?outline:\s*none;/);
  assert.match(css, /\.background-protection-panel__switch-control:hover input:checked \+ \.setting-switch/);
  assert.match(css, /\.background-protection-panel__operation\.is-visible/);
  assert.match(css, /\.background-protection-panel__operation\.is-busy::after/);
  assert.match(css, /@keyframes background-protection-progress/);
  assert.match(css, /\.background-protection-panel__timer/);
  assert.match(css, /transform-box:\s*fill-box/);
  assert.match(css, /\.background-protection-panel__action:focus-visible/);
  assert.match(css, /@media \(max-width: 600px\)[\s\S]*?\.background-protection-panel__summary/);
  const reducedMotion = css.slice(css.indexOf("@media (prefers-reduced-motion: reduce)"));
  assert.match(reducedMotion, /\.background-protection-spinner\s*\{[\s\S]*?animation-duration:\s*2\.4s !important;/);
  assert.match(reducedMotion, /animation-timing-function:\s*linear !important;/);
  assert.doesNotMatch(reducedMotion, /steps\(/);
  assert.doesNotMatch(reducedMotion, /\.background-protection-spinner\s*\{[\s\S]*?animation:\s*none !important;/);
});
