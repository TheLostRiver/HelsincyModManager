import {
  Bell,
  Check,
  Database,
  FileArchive,
  HardDrive,
  MonitorCog,
  RotateCcw,
  Save,
  ShieldCheck,
  SlidersHorizontal,
} from "lucide-react";
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
import { useFeedback } from "../../shared/feedback";
import {
  coreLocales,
  localeMeta,
  resolveCopy,
  useI18n,
  type LocalePreference,
} from "../../shared/i18n";
import { BackgroundProtectionPanel } from "./BackgroundProtectionPanel";
import { DebugLogSettingsPanel } from "./DebugLogSettingsPanel";
import { ModImportSettingsPanel } from "./ModImportSettingsPanel";
import { modStorageCopy } from "./modStorageCopy";
import { ModStorageSettingsPanel } from "./ModStorageSettingsPanel";
import { settingsPageCopy } from "./settingsPageCopy";

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
  const {
    locale,
    preference: localePreference,
    systemLocale,
    setPreference: setLocalePreference,
  } = useI18n();
  const { pushToast } = useFeedback();
  const copy = resolveCopy(settingsPageCopy, locale);
  const storageCopy = resolveCopy(modStorageCopy, locale);
  const prerequisites = useGamePrerequisites("mhw");
  const [windowClosePreference, setWindowClosePreference] = useState<WindowClosePreference>(() =>
    typeof window === "undefined" ? "ask" : loadWindowClosePreference(),
  );
  // 只存"保存失败"这个事实，不存渲染文案：切换界面语言后错误提示必须跟着换语言。
  const [hasWindowClosePreferenceError, setHasWindowClosePreferenceError] = useState(false);

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
      setHasWindowClosePreferenceError(true);
      return;
    }

    setWindowClosePreference(value);
    setHasWindowClosePreferenceError(false);
  };

  const updateLocalePreference = (value: LocalePreference) => {
    if (value === localePreference) {
      return;
    }

    setLocalePreference(value);

    // 确认反馈必须用切换后的语言：用户选了日语，就不该再收到中文 toast。
    const nextLocale = value === "system" ? systemLocale : value;
    const nextCopy = resolveCopy(settingsPageCopy, nextLocale);
    pushToast({
      eventKey: "settings.language-changed",
      title: nextCopy.appearance.language.toastTitle,
      message: nextCopy.appearance.language.toastMessage(localeMeta[nextLocale].nativeName),
      tone: "success",
    });
  };

  // 「跟随系统」置顶并括注当前系统解析结果；语言项永远显示自称名（I18N_DESIGN.md UI 规格）。
  const localeOptions: { value: LocalePreference; label: string }[] = [
    {
      value: "system",
      label: copy.appearance.language.followSystem(localeMeta[systemLocale].nativeName),
    },
    ...coreLocales.map((candidate) => ({
      value: candidate,
      label: localeMeta[candidate].nativeName,
    })),
  ];

  return (
    <section className="settings-page" aria-labelledby="settings-title">
      <header className="settings-hero">
        <div className="settings-hero__copy">
          <span className="settings-hero__eyebrow">{copy.hero.eyebrow}</span>
          <h2 id="settings-title">{copy.hero.title}</h2>
          <p>{copy.hero.description}</p>
        </div>

        <div className="settings-hero__status" aria-label={copy.hero.statusLabel}>
          <span className={`settings-save-indicator ${hasSessionChanges ? "is-dirty" : ""}`}>
            <Save size={14} strokeWidth={2.1} />
            {hasSessionChanges ? copy.hero.dirty : copy.hero.pristine}
          </span>
          <button type="button" className="settings-reset-button" onClick={resetSessionPreview} disabled={!hasSessionChanges}>
            <RotateCcw size={14} strokeWidth={2.1} />
            {copy.hero.reset}
          </button>
        </div>
      </header>

      <div className="settings-sections">
        <SettingsSection
          title={copy.appearance.title}
          description={copy.appearance.description}
          icon={SlidersHorizontal}
          tourId="settings.appearance"
        >
          {/*
           * 主题的唯一常驻入口在这里。顶栏 ThemeMenu 只是纯图标快捷方式，
           * 且视口 <=1060px 时整个 .window-tools 会被隐藏（见 AppFrame.css 的成对断点），
           * 那时设置页是够得到主题的唯一位置。
           */}
          <ChoiceGroup<ColorSchemePreference>
            label={copy.appearance.theme.label}
            hint={copy.appearance.theme.hint}
            value={colorSchemePreference}
            options={[
              { value: "light", label: copy.appearance.theme.light },
              { value: "dark", label: copy.appearance.theme.dark },
              { value: "system", label: copy.appearance.theme.system },
            ]}
            onChange={setColorSchemePreference}
          />
          {/* 语言切换的唯一入口（I18N_DESIGN.md：不做顶栏快捷切换）。 */}
          <ChoiceGroup<LocalePreference>
            label={copy.appearance.language.label}
            hint={copy.appearance.language.hint}
            value={localePreference}
            options={localeOptions}
            onChange={updateLocalePreference}
          />
          <ToggleRow
            title={copy.appearance.compactPanels.title}
            description={copy.appearance.compactPanels.description}
            checked={settings.compactPanels}
            onChange={() => updateToggle("compactPanels")}
          />
          <ToggleRow
            title={copy.appearance.reduceMotion.title}
            description={copy.appearance.reduceMotion.description}
            checked={settings.reduceMotion}
            onChange={() => updateToggle("reduceMotion")}
          />
          <ChoiceGroup
            label={copy.appearance.startPage.label}
            value={settings.startPage}
            options={[
              { value: "dashboard", label: copy.appearance.startPage.dashboard },
              { value: "mods", label: copy.appearance.startPage.mods },
              { value: "last", label: copy.appearance.startPage.last },
            ]}
            onChange={(value) => updateChoice("startPage", value)}
          />
        </SettingsSection>

        <SettingsSection
          title={copy.windowBehavior.title}
          description={copy.windowBehavior.description}
          icon={MonitorCog}
          tourId="settings.window-behavior"
        >
          <ChoiceGroup
            label={copy.windowBehavior.closeLabel}
            value={windowClosePreference}
            options={[
              { value: "ask", label: copy.windowBehavior.ask },
              { value: "tray", label: copy.windowBehavior.tray },
              { value: "exit", label: copy.windowBehavior.exit },
            ]}
            onChange={updateWindowClosePreference}
          />
          {hasWindowClosePreferenceError ? (
            <div className="settings-callout" role="alert">
              <Bell size={16} strokeWidth={2.1} />
              <span>{copy.windowBehavior.saveError}</span>
            </div>
          ) : null}
          <div className="settings-callout settings-callout--neutral" role="note">
            <Bell size={16} strokeWidth={2.1} />
            <span>{copy.windowBehavior.note}</span>
          </div>
        </SettingsSection>
        <SettingsSection
          title={copy.modImport.title}
          description={copy.modImport.description}
          icon={FileArchive}
        >
          <ModImportSettingsPanel />
          <ToggleRow
            title={copy.modImport.previewAfterImport.title}
            description={copy.modImport.previewAfterImport.description}
            checked={settings.previewAfterImport}
            onChange={() => updateToggle("previewAfterImport")}
          />
          <ToggleRow
            title={copy.modImport.confirmBeforeConflict.title}
            description={copy.modImport.confirmBeforeConflict.description}
            checked={settings.confirmBeforeConflict}
            onChange={() => updateToggle("confirmBeforeConflict")}
          />
        </SettingsSection>

        <SettingsSection
          title={storageCopy.section.title}
          description={storageCopy.section.description}
          icon={HardDrive}
          tourId="settings.mod-storage"
        >
          <ModStorageSettingsPanel />
        </SettingsSection>

        <SettingsSection
          title={copy.prerequisites.title}
          description={copy.prerequisites.description}
          icon={ShieldCheck}
          tourId="settings.prerequisites"
        >
          <GamePrerequisitePanel state={prerequisites.state} onRefresh={prerequisites.refresh} variant="embedded" />
        </SettingsSection>

        <SettingsSection
          title={copy.saveBackup.title}
          description={copy.saveBackup.description}
          icon={ShieldCheck}
          tourId="settings.save-backup"
        >
          <BackgroundProtectionPanel />
          <ToggleRow
            title={copy.saveBackup.backupReminder.title}
            description={copy.saveBackup.backupReminder.description}
            checked={settings.backupReminder}
            onChange={() => updateToggle("backupReminder")}
          />
        </SettingsSection>

        <SettingsSection
          title={copy.logs.title}
          description={copy.logs.description}
          icon={Database}
        >
          <DebugLogSettingsPanel />
          <div className="settings-callout" role="note">
            <Bell size={16} strokeWidth={2.1} />
            <span>{copy.logs.exportNote}</span>
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
