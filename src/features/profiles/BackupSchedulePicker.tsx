import { CalendarDays, Check, Clock3 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent, WheelEvent as ReactWheelEvent } from "react";
import type { ProfileBackupScheduleDto } from "./profileSaveSettingsTypes";
import { defaultSchedule, formatBackupSchedule } from "./profileViewModel";

const hours = Array.from({ length: 24 }, (_, index) => index);
const minutes = Array.from({ length: 60 }, (_, index) => index);
const weekdays = [
  { value: 1, label: "星期一" },
  { value: 2, label: "星期二" },
  { value: 3, label: "星期三" },
  { value: 4, label: "星期四" },
  { value: 5, label: "星期五" },
  { value: 6, label: "星期六" },
  { value: 0, label: "星期日" },
];
const weekdayOrder = new Map(weekdays.map((day, index) => [day.value, index]));
const scrollPickerItemHeight = 38;
const scrollPickerDisplayOffsets = [-2, -1, 0, 1, 2];

function wrapIndex(index: number, length: number) {
  if (length <= 0) return 0;
  return ((index % length) + length) % length;
}

function getWheelItemStyle(offset: number) {
  const distance = Math.abs(offset);
  return {
    opacity: 1 - distance * 0.2,
    transform: `translateY(${offset * scrollPickerItemHeight}px) rotateX(${-offset * 28}deg) scale(${1 - distance * 0.08})`,
    zIndex: 10 - distance,
  };
}

type BackupSchedulePickerProps = {
  schedule: ProfileBackupScheduleDto;
  onChange: (schedule: ProfileBackupScheduleDto) => void;
  disabled?: boolean;
};

export function BackupSchedulePicker({ schedule, onChange, disabled = false }: BackupSchedulePickerProps) {
  const [open, setOpen] = useState(false);
  const wrapperRef = useRef<HTMLDivElement>(null);
  const usesTime = schedule.cadence !== "manual" && !disabled;

  useEffect(() => {
    if (!open) return;

    const handlePointerDown = (event: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

  const updateTime = (hour: number, minute: number) => {
    onChange({ ...schedule, hour, minute });
  };

  const toggleWeekday = (day: number) => {
    const nextDays = schedule.weekdays.includes(day)
      ? schedule.weekdays.filter((value) => value !== day)
      : [...schedule.weekdays, day].sort(
          (a, b) => (weekdayOrder.get(a) ?? 0) - (weekdayOrder.get(b) ?? 0),
        );
    onChange({ ...schedule, weekdays: nextDays.length > 0 ? nextDays : [day] });
  };

  return (
    <div className="backup-schedule-picker" ref={wrapperRef}>
      <div className="backup-schedule-picker__segments" role="group" aria-label="备份频率">
        {(["manual", "daily", "weekly"] as const).map((cadence) => (
          <button
            key={cadence}
            type="button"
            className={schedule.cadence === cadence ? "is-selected" : ""}
            aria-pressed={schedule.cadence === cadence}
            disabled={disabled}
            onClick={() => {
              onChange(defaultSchedule(cadence));
              setOpen(cadence !== "manual");
            }}
          >
            {schedule.cadence === cadence ? <Check size={13} /> : null}
            {cadence === "manual" ? "手动" : cadence === "daily" ? "每日" : "每周"}
          </button>
        ))}
      </div>

      <button
        type="button"
        className="backup-schedule-picker__trigger"
        aria-expanded={open}
        onClick={() => setOpen((current) => (usesTime ? !current : false))}
        disabled={disabled || !usesTime}
      >
        {schedule.cadence === "weekly" ? <CalendarDays size={15} /> : <Clock3 size={15} />}
        {formatBackupSchedule(schedule)}
      </button>

      {open && usesTime ? (
        <div className="backup-schedule-popover" role="dialog" aria-label="自动备份时间">
          {schedule.cadence === "weekly" ? (
            <div className="backup-weekday-row" role="group" aria-label="每周日期">
              {weekdays.map((day) => (
                <button
                  key={day.value}
                  type="button"
                  className={schedule.weekdays.includes(day.value) ? "is-selected" : ""}
                  aria-pressed={schedule.weekdays.includes(day.value)}
                  disabled={disabled}
                  onClick={() => toggleWeekday(day.value)}
                >
                  {day.label}
                </button>
              ))}
            </div>
          ) : null}

          <div className="backup-time-pickers">
            <ScrollPicker
              values={hours}
              value={schedule.hour ?? 3}
              suffix="时"
              disabled={disabled}
              onChange={(hour) => updateTime(hour, schedule.minute ?? 0)}
            />
            <span className="backup-time-colon" aria-hidden="true">
              :
            </span>
            <ScrollPicker
              values={minutes}
              value={schedule.minute ?? 0}
              suffix="分"
              disabled={disabled}
              onChange={(minute) => updateTime(schedule.hour ?? 3, minute)}
            />
          </div>

          <button
            type="button"
            className="backup-schedule-popover__done"
            onClick={() => setOpen(false)}
          >
            确定
          </button>
        </div>
      ) : null}
    </div>
  );
}

function ScrollPicker({
  values,
  value,
  suffix,
  disabled,
  onChange,
}: {
  values: number[];
  value: number;
  suffix: string;
  disabled: boolean;
  onChange: (value: number) => void;
}) {
  const selectedIndex = useMemo(() => Math.max(values.indexOf(value), 0), [value, values]);
  const visibleItems = useMemo(
    () =>
      scrollPickerDisplayOffsets.map((offset) => {
        const index = wrapIndex(selectedIndex + offset, values.length);
        return {
          index,
          item: values[index] ?? values[0] ?? 0,
          offset,
        };
      }),
    [selectedIndex, values],
  );
  const dragStateRef = useRef<{ pointerId: number; startY: number; startIndex: number } | null>(null);
  const draggedRef = useRef(false);
  const [dragging, setDragging] = useState(false);

  const updateByIndex = (index: number) => {
    const nextIndex = wrapIndex(index, values.length);
    const nextValue = values[nextIndex];
    if (nextValue !== undefined && nextValue !== value) {
      onChange(nextValue);
    }
  };

  const handleWheel = (event: ReactWheelEvent<HTMLDivElement>) => {
    if (disabled) return;
    event.preventDefault();
    updateByIndex(selectedIndex + (event.deltaY > 0 ? 1 : -1));
  };

  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (disabled || event.button !== 0) return;
    dragStateRef.current = {
      pointerId: event.pointerId,
      startY: event.clientY,
      startIndex: selectedIndex,
    };
    draggedRef.current = false;
    event.currentTarget.setPointerCapture(event.pointerId);
    setDragging(true);
  };

  const handlePointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const dragState = dragStateRef.current;
    if (!dragState || dragState.pointerId !== event.pointerId) return;
    event.preventDefault();
    const delta = dragState.startY - event.clientY;
    if (Math.abs(delta) >= 4) {
      draggedRef.current = true;
    }
    updateByIndex(dragState.startIndex + Math.round(delta / scrollPickerItemHeight));
  };

  const finishDrag = (event: ReactPointerEvent<HTMLDivElement>) => {
    const dragState = dragStateRef.current;
    if (!dragState || dragState.pointerId !== event.pointerId) return;
    dragStateRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    setDragging(false);
  };

  return (
    <div
      className={`scroll-picker-wrapper ${dragging ? "is-dragging" : ""}`}
      role="listbox"
      aria-label={suffix}
      onClickCapture={(event) => {
        if (!draggedRef.current) return;
        event.preventDefault();
        event.stopPropagation();
        draggedRef.current = false;
      }}
      onPointerCancel={finishDrag}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={finishDrag}
      onWheel={handleWheel}
    >
      <div className="scroll-picker-list">
        {visibleItems.map(({ index, item, offset }) => (
          <button
            key={index}
            type="button"
            className={`scroll-picker-item ${offset === 0 ? "is-selected" : ""}`}
            aria-selected={offset === 0}
            disabled={disabled}
            onClick={() => onChange(item)}
            style={getWheelItemStyle(offset)}
          >
            {String(item).padStart(2, "0")}
            <span>{suffix}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
