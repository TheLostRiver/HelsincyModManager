import { Bell, Check, Database, FileArchive, MonitorCog, RotateCcw, Save, ShieldCheck, SlidersHorizontal } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ComponentType, type ReactNode } from "react";
import { GamePrerequisitePanel } from "../game-setup/GamePrerequisitePanel";
import { useGamePrerequisites } from "../game-setup/useGamePrerequisites";
import {
  loadWindowClosePreference,
  saveWindowClosePreference,
  type WindowClosePreference,
} from "../../app/window-lifecycle/windowClosePreference";

type ToggleSettingId =
  | "compactPanels"
  | "reduceMotion"
  | "previewAfterImport"
  | "confirmBeforeConflict"
  | "backupReminder"
  | "diagnosticDetails";

type SettingsState = Record<ToggleSettingId, boolean> & {
  startPage: "dashboard" | "mods" | "last";
  backupCadence: "manual" | "daily" | "weekly";
  dailyBackupHour: number;
  dailyBackupMinute: number;
  weeklyBackupHour: number;
  weeklyBackupMinute: number;
  weeklyBackupDays: number[];
};

const BACKUP_HOURS = Array.from({ length: 24 }, (_, i) => i);
const BACKUP_MINUTES = Array.from({ length: 60 }, (_, i) => i);
const WEEKDAYS = [
  { value: 1, label: "周一" },
  { value: 2, label: "周二" },
  { value: 3, label: "周三" },
  { value: 4, label: "周四" },
  { value: 5, label: "周五" },
  { value: 6, label: "周六" },
  { value: 0, label: "周日" },
];
const pad2 = (n: number) => String(n).padStart(2, "0");

const formatWeeklyDays = (days: number[]) => {
  if (days.length === 7) return "每天";
  if (days.length === 0) return "每周";
  const sortedDays = [...days].sort((a, b) => {
    const adjA = a === 0 ? 7 : a;
    const adjB = b === 0 ? 7 : b;
    return adjA - adjB;
  });
  const dayNames = sortedDays
    .map((d) => WEEKDAYS.find((w) => w.value === d)?.label.replace("周", ""))
    .filter(Boolean)
    .join("、");
  return `每周${dayNames}`;
};

type SettingSectionProps = {
  title: string;
  description: string;
  icon: ComponentType<{ size?: number; strokeWidth?: number }>;
  children: ReactNode;
};

const initialSettings: SettingsState = {
  compactPanels: false,
  reduceMotion: false,
  previewAfterImport: true,
  confirmBeforeConflict: true,
  backupReminder: true,
  diagnosticDetails: false,
  startPage: "dashboard",
  backupCadence: "manual",
  dailyBackupHour: 3,
  dailyBackupMinute: 0,
  weeklyBackupHour: 3,
  weeklyBackupMinute: 0,
  weeklyBackupDays: [0],
};

export function SettingsPage() {
  const [settings, setSettings] = useState<SettingsState>(initialSettings);
  const [isTimePickerOpen, setIsTimePickerOpen] = useState(false);
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
            这些选项现在只在本次会话中交互预览。后续接入正式设置存储后，会通过统一配置服务保存。
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
          description="控制工作台和列表页的显示密度。正式保存前不会写入配置文件。"
          icon={SlidersHorizontal}
        >
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
            <span>当前真正后台守护尚未落地；选择退出应用后，客户端运行期自动备份不会继续检查。</span>
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
        >
          <GamePrerequisitePanel state={prerequisites.state} onRefresh={prerequisites.refresh} variant="embedded" />
        </SettingsSection>

        <SettingsSection
          title="存档备份"
          description="当前不选择目录、不读取真实存档。这里仅预览未来备份策略入口。"
          icon={ShieldCheck}
        >
          <ToggleRow
            title="安装前提醒备份"
            description="在执行会写入游戏目录的任务前提示检查存档备份状态。"
            checked={settings.backupReminder}
            onChange={() => updateToggle("backupReminder")}
          />
          <div className="backup-cadence-wrapper" style={{ position: "relative" }}>
            <ChoiceGroup
              label="自动备份节奏"
              value={settings.backupCadence}
              options={[
                { value: "manual", label: "仅手动" },
                { 
                  value: "daily", 
                  label: settings.backupCadence === "daily" 
                    ? `每日 ${pad2(settings.dailyBackupHour)}:${pad2(settings.dailyBackupMinute)}` 
                    : "每日" 
                },
                { 
                  value: "weekly", 
                  label: settings.backupCadence === "weekly" 
                    ? `${formatWeeklyDays(settings.weeklyBackupDays)} ${pad2(settings.weeklyBackupHour)}:${pad2(settings.weeklyBackupMinute)}` 
                    : "每周" 
                },
              ]}
              onChange={(value) => {
                updateChoice("backupCadence", value);
                if (value !== "manual") {
                  setIsTimePickerOpen(true);
                } else {
                  setIsTimePickerOpen(false);
                }
              }}
            />
            {isTimePickerOpen && settings.backupCadence !== "manual" && (
              <TimePickerPopover
                key={settings.backupCadence}
                initialHour={settings.backupCadence === "daily" ? settings.dailyBackupHour : settings.weeklyBackupHour}
                initialMinute={settings.backupCadence === "daily" ? settings.dailyBackupMinute : settings.weeklyBackupMinute}
                onSave={(h, m) => {
                  if (settings.backupCadence === "daily") {
                    updateChoice("dailyBackupHour", h);
                    updateChoice("dailyBackupMinute", m);
                  } else {
                    updateChoice("weeklyBackupHour", h);
                    updateChoice("weeklyBackupMinute", m);
                  }
                  setIsTimePickerOpen(false);
                }}
                onClose={() => setIsTimePickerOpen(false)}
              />
            )}
          </div>
          {settings.backupCadence !== "manual" && (
            <div className="backup-schedule-detail">
              {settings.backupCadence === "weekly" && (
                <div className="backup-schedule-row">
                  <span className="backup-schedule-row__label">备份日</span>
                  <div className="backup-day-options">
                    {WEEKDAYS.map((day) => {
                      const isSelected = settings.weeklyBackupDays.includes(day.value);
                      return (
                        <button
                          key={day.value}
                          type="button"
                          className={`backup-day-btn ${isSelected ? "is-selected" : ""}`}
                          aria-pressed={isSelected}
                          onClick={() => {
                            const current = settings.weeklyBackupDays;
                            if (isSelected) {
                              if (current.length > 1) {
                                updateChoice("weeklyBackupDays", current.filter((d) => d !== day.value));
                              }
                            } else {
                              updateChoice("weeklyBackupDays", [...current, day.value]);
                            }
                          }}
                        >
                          {day.label}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>
          )}
        </SettingsSection>

        <SettingsSection
          title="日志与诊断"
          description="诊断包导出需要后端脱敏能力，本页不会生成或写入任何日志文件。"
          icon={Database}
        >
          <ToggleRow
            title="详细诊断模式"
            description="预留给未来任务日志筛选和诊断摘要，不包含真实路径或第三方 Mod 内容。"
            checked={settings.diagnosticDetails}
            onChange={() => updateToggle("diagnosticDetails")}
          />
          <div className="settings-callout" role="note">
            <Bell size={16} strokeWidth={2.1} />
            <span>正式导出前必须经过统一脱敏，并由用户主动触发。</span>
          </div>
        </SettingsSection>
      </div>
    </section>
  );
}

function SettingsSection({ title, description, icon: Icon, children }: SettingSectionProps) {
  return (
    <article className="settings-section">
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
  value,
  options,
  onChange,
}: {
  label: string;
  value: TValue;
  options: { value: TValue; label: string }[];
  onChange: (value: TValue) => void;
}) {
  return (
    <fieldset className="setting-choice-group">
      <legend>{label}</legend>
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

function TimePickerPopover({
  initialHour,
  initialMinute,
  onSave,
  onClose,
}: {
  initialHour: number;
  initialMinute: number;
  onSave: (h: number, m: number) => void;
  onClose: () => void;
}) {
  const [hour, setHour] = useState(initialHour);
  const [minute, setMinute] = useState(initialMinute);
  const popoverRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleOutsideClick = (e: MouseEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handleOutsideClick);
    return () => document.removeEventListener("mousedown", handleOutsideClick);
  }, [onClose]);

  return (
    <div className="backup-time-popover" ref={popoverRef}>
      <div className="backup-time-pickers">
        <ScrollPicker values={BACKUP_HOURS} value={hour} onChange={setHour} suffix="时" />
        <span className="backup-time-colon" aria-hidden="true">:</span>
        <ScrollPicker values={BACKUP_MINUTES} value={minute} onChange={setMinute} suffix="分" />
      </div>
      <div className="backup-time-popover__footer">
        <button type="button" className="backup-time-popover__btn" onClick={() => onSave(hour, minute)}>
          确定
        </button>
      </div>
    </div>
  );
}

function ScrollPicker({
  values,
  value,
  onChange,
  suffix,
}: {
  values: number[];
  value: number;
  onChange: (value: number) => void;
  suffix: string;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const isScrolling = useRef(false);
  const scrollTimeout = useRef<ReturnType<typeof window.setTimeout> | undefined>(undefined);
  
  const REPEAT_COUNT = 5;
  const CENTER_SET = 2;
  const repeatedValues = useMemo(() => Array.from({ length: REPEAT_COUNT }, () => values).flat(), [values]);

  const getCenterIdx = (val: number) => {
    const realIdx = values.indexOf(val);
    return realIdx !== -1 ? realIdx + values.length * CENTER_SET : 0;
  };

  const [activeIndex, setActiveIndex] = useState(() => getCenterIdx(value));

  useEffect(() => {
    return () => {
      clearTimeout(scrollTimeout.current);
    };
  }, []);

  useEffect(() => {
    if (!isScrolling.current) {
      const realIdx = values.indexOf(value);
      if (realIdx !== -1) {
        const currentRealIdx = activeIndex % values.length;
        if (currentRealIdx !== realIdx) {
          const newIdx = realIdx + values.length * CENTER_SET;
          setActiveIndex(newIdx);
          if (containerRef.current) {
            containerRef.current.scrollTop = newIdx * 38;
          }
        }
      }
    }
  }, [value, values, activeIndex]);

  useEffect(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = activeIndex * 38;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const top = e.currentTarget.scrollTop;
    const idx = Math.max(0, Math.min(repeatedValues.length - 1, Math.round(top / 38)));
    
    if (idx !== activeIndex) {
      setActiveIndex(idx);
    }

    isScrolling.current = true;
    clearTimeout(scrollTimeout.current);
    scrollTimeout.current = setTimeout(() => {
      isScrolling.current = false;
      const realVal = repeatedValues[idx];
      onChange(realVal);

      if (idx < values.length || idx >= values.length * (REPEAT_COUNT - 1)) {
        const realIdx = idx % values.length;
        const centerIdx = realIdx + values.length * CENTER_SET;
        if (containerRef.current) {
          containerRef.current.scrollTop = centerIdx * 38;
          setActiveIndex(centerIdx);
        }
      }
    }, 150);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      const next = Math.min(repeatedValues.length - 1, activeIndex + 1);
      if (containerRef.current) containerRef.current.scrollTo({ top: next * 38, behavior: "smooth" });
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      const next = Math.max(0, activeIndex - 1);
      if (containerRef.current) containerRef.current.scrollTo({ top: next * 38, behavior: "smooth" });
    }
  };

  return (
    <div className="scroll-picker-wrapper" aria-label={suffix}>
      <div className="scroll-picker__highlight" aria-hidden="true" />
      <div
        className="scroll-picker"
        ref={containerRef}
        onScroll={handleScroll}
        onKeyDown={handleKeyDown}
        tabIndex={0}
        role="listbox"
      >
        <div className="scroll-picker__spacer" aria-hidden="true" />
        {repeatedValues.map((v, idx) => {
          const offset = Math.abs(idx - activeIndex);
          const dataOffset = offset > 2 ? 3 : offset;
          return (
            <div
              key={`${idx}-${v}`}
              className="scroll-picker__item"
              data-offset={dataOffset}
              aria-selected={idx === activeIndex}
              role="option"
              onClick={() => {
                if (containerRef.current) {
                  containerRef.current.scrollTo({ top: idx * 38, behavior: "smooth" });
                }
              }}
            >
              <span className="scroll-picker__value">{pad2(v)}</span>
              <span className="scroll-picker__suffix" style={{ opacity: idx === activeIndex ? 1 : 0 }}>
                {suffix}
              </span>
            </div>
          );
        })}
        <div className="scroll-picker__spacer" aria-hidden="true" />
      </div>
    </div>
  );
}
