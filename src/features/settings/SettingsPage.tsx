import { Bell, Check, Database, FileArchive, MonitorCog, RotateCcw, Save, ShieldCheck, SlidersHorizontal } from "lucide-react";
import { useId, useMemo, useState, type ComponentType, type ReactNode } from "react";
import type { ColorSchemePreference } from "../../app/appearance/colorSchemeTypes";
import { useColorScheme } from "../../app/appearance/useColorScheme";
import { GamePrerequisitePanel } from "../game-setup/GamePrerequisitePanel";
import { useGamePrerequisites } from "../game-setup/useGamePrerequisites";
import {
  loadWindowClosePreference,
  saveWindowClosePreference,
  type WindowClosePreference,
} from "../../app/window-lifecycle/windowClosePreference";
import { BackgroundProtectionPanel } from "./BackgroundProtectionPanel";
import { DebugLogSettingsPanel } from "./DebugLogSettingsPanel";

type ToggleSettingId =
  | "compactPanels"
  | "reduceMotion"
  | "previewAfterImport"
  | "confirmBeforeConflict"
  | "backupReminder";

type SettingsState = Record<ToggleSettingId, boolean> & {
  startPage: "dashboard" | "mods" | "last";
};

type SettingSectionProps = {
  title: string;
  description: string;
  icon: ComponentType<{ size?: number; strokeWidth?: number }>;
  tourId?: string;
  children: ReactNode;
};

const initialSettings: SettingsState = {
  compactPanels: false,
  reduceMotion: false,
  previewAfterImport: true,
  confirmBeforeConflict: true,
  backupReminder: true,
  startPage: "dashboard",
};

export function SettingsPage() {
  const [settings, setSettings] = useState<SettingsState>(initialSettings);
  const { preference: colorSchemePreference, setPreference: setColorSchemePreference } =
    useColorScheme();
  const prerequisites = useGamePrerequisites("mhw");
  const [windowClosePreference, setWindowClosePreference] = useState<WindowClosePreference>(() =>
    typeof window === "undefined" ? "ask" : loadWindowClosePreference(),
  );
  const [windowClosePreferenceError, setWindowClosePreferenceError] = useState<string | null>(null);

  const hasSessionChanges = useMemo(
    () => JSON.stringify(settings) !== JSON.stringify(initialSettings),
    [settings],
  );

  const updateToggle = (id: ToggleSettingId) => {
    setSettings((current) => ({ ...current, [id]: !current[id] }));
  };

  const updateChoice = <TKey extends keyof SettingsState>(key: TKey, value: SettingsState[TKey]) => {
    setSettings((current) => ({ ...current, [key]: value }));
  };

  const resetSessionPreview = () => {
    setSettings(initialSettings);
  };

  const updateWindowClosePreference = (value: WindowClosePreference) => {
    const saveSucceeded = saveWindowClosePreference(undefined, value);
    if (!saveSucceeded) {
      setWindowClosePreferenceError("关闭行为偏好保存失败，请检查应用存储权限后重试。");
      return;
    }

    setWindowClosePreference(value);
    setWindowClosePreferenceError(null);
  };

  return (
    <section className="settings-page" aria-labelledby="settings-title">
      <header className="settings-hero">
        <div className="settings-hero__copy">
          <span className="settings-hero__eyebrow">应用设置</span>
          <h2 id="settings-title">调整管理器的工作方式</h2>
          <p>
            后台保护与窗口关闭偏好会正式保存；其余标记为预览的选项只在当前会话中生效。
          </p>
        </div>

        <div className="settings-hero__status" aria-label="设置保存状态">
          <span className={`settings-save-indicator ${hasSessionChanges ? "is-dirty" : ""}`}>
            <Save size={14} strokeWidth={2.1} />
            {hasSessionChanges ? "存在本次会话改动" : "使用默认预览值"}
          </span>
          <button type="button" className="settings-reset-button" onClick={resetSessionPreview} disabled={!hasSessionChanges}>
            <RotateCcw size={14} strokeWidth={2.1} />
            重置预览
          </button>
        </div>
      </header>

      <div className="settings-sections">
        <SettingsSection
          title="界面偏好"
          description="主题模式会立即保存并长期生效；其余显示密度类选项只是本次会话的预览，正式保存前不写入配置文件。"
          icon={SlidersHorizontal}
          tourId="settings.appearance"
        >
          {/*
           * 主题的唯一常驻入口在这里。顶栏 ThemeMenu 只是纯图标快捷方式，
           * 且视口 <=1060px 时整个 .window-tools 会被隐藏（见 AppFrame.css 的成对断点），
           * 那时设置页是够得到主题的唯一位置。
           */}
          <ChoiceGroup<ColorSchemePreference>
            label="主题模式"
            hint="立即生效并长期保存，不受下方预览选项的重置影响。"
            value={colorSchemePreference}
            options={[
              { value: "light", label: "浅色模式" },
              { value: "dark", label: "深色模式" },
              { value: "system", label: "跟随系统" },
            ]}
            onChange={setColorSchemePreference}
          />
          <ToggleRow
            title="紧凑面板"
            description="减少卡片内边距，适合小窗口或 Steam Deck 桌面模式。"
            checked={settings.compactPanels}
            onChange={() => updateToggle("compactPanels")}
          />
          <ToggleRow
            title="减少动效"
            description="降低页面过渡和 hover 动画强度。未来应与系统无障碍偏好合并。"
            checked={settings.reduceMotion}
            onChange={() => updateToggle("reduceMotion")}
          />
          <ChoiceGroup
            label="启动后打开"
            value={settings.startPage}
            options={[
              { value: "dashboard", label: "工作台" },
              { value: "mods", label: "Mod 管理" },
              { value: "last", label: "上次页面" },
            ]}
            onChange={(value) => updateChoice("startPage", value)}
          />
        </SettingsSection>

        <SettingsSection
          title="窗口行为"
          description="控制点击窗口关闭按钮时的默认动作；这不会改变后台守护是否已启用。"
          icon={MonitorCog}
          tourId="settings.window-behavior"
        >
          <ChoiceGroup
            label="关闭主窗口时"
            value={windowClosePreference}
            options={[
              { value: "ask", label: "每次询问" },
              { value: "tray", label: "收起至托盘" },
              { value: "exit", label: "退出应用" },
            ]}
            onChange={updateWindowClosePreference}
          />
          {windowClosePreferenceError ? (
            <div className="settings-callout" role="alert">
              <Bell size={16} strokeWidth={2.1} />
              <span>{windowClosePreferenceError}</span>
            </div>
          ) : null}
          <div className="settings-callout settings-callout--neutral" role="note">
            <Bell size={16} strokeWidth={2.1} />
            <span>关闭行为偏好与后台保护是独立设置；退出后的保护状态以“存档备份”区域为准。</span>
          </div>
        </SettingsSection>
        <SettingsSection
          title="Mod 导入"
          description="这些选项只影响未来导入流程的前端意图表达，不在前端判断文件安全。"
          icon={FileArchive}
        >
          <ToggleRow
            title="导入后显示预览"
            description="导入完成后优先展示预览图和结构摘要。预览图校验仍应由后端完成。"
            checked={settings.previewAfterImport}
            onChange={() => updateToggle("previewAfterImport")}
          />
          <ToggleRow
            title="冲突前二次确认"
            description="当安装计划存在冲突时，在继续前显示确认步骤。"
            checked={settings.confirmBeforeConflict}
            onChange={() => updateToggle("confirmBeforeConflict")}
          />
        </SettingsSection>

        <SettingsSection
          title="前置环境"
          description="只读检查当前已配置游戏目录中的 Stracker's Loader 与 CRCBypass，不访问测试目录。"
          icon={ShieldCheck}
          tourId="settings.prerequisites"
        >
          <GamePrerequisitePanel state={prerequisites.state} onRefresh={prerequisites.refresh} variant="embedded" />
        </SettingsSection>

        <SettingsSection
          title="存档备份"
          description="后台保护会正式保存；安装前提醒仍是当前会话预览，不读取真实存档。"
          icon={ShieldCheck}
          tourId="settings.save-backup"
        >
          <BackgroundProtectionPanel />
          <ToggleRow
            title="安装前提醒备份"
            description="在执行会写入游戏目录的任务前提示检查存档备份状态。"
            checked={settings.backupReminder}
            onChange={() => updateToggle("backupReminder")}
          />
        </SettingsSection>

        <SettingsSection
          title="日志与诊断"
          description="诊断包导出需要后端脱敏能力，本页不会生成或写入任何日志文件。"
          icon={Database}
        >
          <DebugLogSettingsPanel />
          <div className="settings-callout" role="note">
            <Bell size={16} strokeWidth={2.1} />
            <span>正式导出前必须经过统一脱敏，并由用户主动触发。</span>
          </div>
        </SettingsSection>
      </div>
    </section>
  );
}

function SettingsSection({ title, description, icon: Icon, tourId, children }: SettingSectionProps) {
  return (
    <article className="settings-section" data-tour-id={tourId}>
      <header className="settings-section__header">
        <div className="settings-section__icon" aria-hidden="true">
          <Icon size={18} strokeWidth={2} />
        </div>
        <div className="settings-section__copy">
          <h3>{title}</h3>
          <p>{description}</p>
        </div>
      </header>
      <div className="settings-section__body">{children}</div>
    </article>
  );
}

function ToggleRow({
  title,
  description,
  checked,
  onChange,
}: {
  title: string;
  description: string;
  checked: boolean;
  onChange: () => void;
}) {
  return (
    <label className="setting-row">
      <span className="setting-row__copy">
        <strong>{title}</strong>
        <span>{description}</span>
      </span>
      <input type="checkbox" checked={checked} onChange={onChange} />
      <span className="setting-switch" aria-hidden="true" />
    </label>
  );
}

function ChoiceGroup<TValue extends string>({
  label,
  hint,
  value,
  options,
  onChange,
}: {
  label: string;
  hint?: string;
  value: TValue;
  options: { value: TValue; label: string }[];
  onChange: (value: TValue) => void;
}) {
  // hint 承载"立即生效并长期保存"这类关键语义，必须和选项组建立可编程关联，
  // 否则读屏用户只听得到 legend。
  const hintId = useId();

  return (
    <fieldset
      className="setting-choice-group"
      aria-describedby={hint === undefined ? undefined : hintId}
    >
      <legend>{label}</legend>
      {hint === undefined ? null : (
        <p className="setting-choice-group__hint" id={hintId}>
          {hint}
        </p>
      )}
      <div className="setting-choice-group__options">
        {options.map((option) => (
          <button
            key={option.value}
            type="button"
            className={option.value === value ? "is-selected" : ""}
            aria-pressed={option.value === value}
            onClick={() => onChange(option.value)}
          >
            <Check size={14} strokeWidth={2.5} />
            {option.label}
          </button>
        ))}
      </div>
    </fieldset>
  );
}
