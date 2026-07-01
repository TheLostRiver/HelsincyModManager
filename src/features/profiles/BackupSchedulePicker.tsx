import { CalendarDays, ChevronDown, ChevronUp, Clock3, PauseCircle, Star } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";
import type { ProfileBackupScheduleDto } from "./profileSaveSettingsTypes";
import { defaultSchedule } from "./profileViewModel";

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

function formatWeeklyDaysAbbr(days: number[]) {
  if (!days || days.length === 0) return "周日";
  const map: Record<number, string> = { 1: "一", 2: "二", 3: "三", 4: "四", 5: "五", 6: "六", 0: "日" };
  const sorted = [...days].sort((a, b) => {
    const oa = weekdayOrder.get(a) ?? 0;
    const ob = weekdayOrder.get(b) ?? 0;
    return oa - ob;
  });
  return "周" + sorted.map((day) => map[day] || "").join(",");
}

function formatTime(hour: number | null | undefined, minute: number | null | undefined) {
  return `${String(hour ?? 3).padStart(2, "0")}:${String(minute ?? 0).padStart(2, "0")}`;
}

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
  const dailyChipTime = schedule.cadence === "daily" ? formatTime(schedule.hour, schedule.minute) : "03:00";
  const weeklyChipTime = schedule.cadence === "weekly" ? formatTime(schedule.hour, schedule.minute) : "03:00";

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
      <div className="chip-deck">
        <button
          type="button"
          className={`schedule-chip ${schedule.cadence === "manual" ? "is-selected" : ""}`}
          aria-pressed={schedule.cadence === "manual"}
          disabled={disabled}
          onClick={() => {
            onChange(defaultSchedule("manual"));
            setOpen(false);
          }}
        >
          <PauseCircle size={14} />
          <span>手动</span>
        </button>

        <button
          type="button"
          className={`schedule-chip ${schedule.cadence === "daily" ? "is-selected" : ""}`}
          aria-pressed={schedule.cadence === "daily"}
          disabled={disabled}
          onClick={() => {
            if (schedule.cadence !== "daily") {
              onChange(defaultSchedule("daily"));
              setOpen(true);
              return;
            }
            setOpen((current) => !current);
          }}
        >
          <Clock3 size={14} />
          <span>每日备份</span>
          <span className="schedule-chip-sub">{dailyChipTime}</span>
        </button>

        <button
          type="button"
          className={`schedule-chip ${schedule.cadence === "weekly" ? "is-selected" : ""}`}
          aria-pressed={schedule.cadence === "weekly"}
          disabled={disabled}
          onClick={() => {
            if (schedule.cadence !== "weekly") {
              onChange(defaultSchedule("weekly"));
              setOpen(true);
              return;
            }
            setOpen((current) => !current);
          }}
        >
          <CalendarDays size={14} />
          <span>每周备份</span>
          <span className="schedule-chip-sub">
            {formatWeeklyDaysAbbr(schedule.weekdays)} {weeklyChipTime}
          </span>
        </button>
      </div>
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
                  <Star size={12} fill={schedule.weekdays.includes(day.value) ? "currentColor" : "none"} />
                  <span>{day.label.slice(-1)}</span>
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
  const wheelRef = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState(false);
  const [dragOffset, setDragOffset] = useState(0);

  const updateByIndex = useCallback((index: number) => {
    const nextIndex = wrapIndex(index, values.length);
    const nextValue = values[nextIndex];
    if (nextValue !== undefined && nextValue !== value) {
      onChange(nextValue);
    }
  }, [onChange, value, values]);

  const lastWheelTimeRef = useRef(0);
  const handleWheel = useCallback((event: WheelEvent) => {
    if (disabled) return;
    event.preventDefault();

    const now = Date.now();
    if (now - lastWheelTimeRef.current < 120) {
      return;
    }
    lastWheelTimeRef.current = now;

    updateByIndex(selectedIndex + (event.deltaY > 0 ? 1 : -1));
  }, [disabled, selectedIndex, updateByIndex]);

  useEffect(() => {
    const node = wheelRef.current;
    if (!node) return;

    node.addEventListener("wheel", handleWheel, { passive: false });
    return () => {
      node.removeEventListener("wheel", handleWheel);
    };
  }, [handleWheel]);

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
    setDragOffset(0);
  };

  const handlePointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const dragState = dragStateRef.current;
    if (!dragState || dragState.pointerId !== event.pointerId) return;
    event.preventDefault();
    const delta = dragState.startY - event.clientY;
    if (Math.abs(delta) >= 4) {
      draggedRef.current = true;
    }
    const steps = Math.round(delta / scrollPickerItemHeight);
    updateByIndex(dragState.startIndex + steps);
    setDragOffset(delta - steps * scrollPickerItemHeight);
  };

  const finishDrag = (event: ReactPointerEvent<HTMLDivElement>) => {
    const dragState = dragStateRef.current;
    if (!dragState || dragState.pointerId !== event.pointerId) return;
    dragStateRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    setDragging(false);
    setDragOffset(0);
  };

  return (
    <div className="scroll-picker-container">
      <button
        type="button"
        className="scroll-picker-arrow is-up"
        onClick={() => updateByIndex(selectedIndex - 1)}
        disabled={disabled}
        aria-label={`减少${suffix}`}
      >
        <ChevronUp size={15} />
      </button>

      <div
        ref={wheelRef}
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
      >
        <div className="scroll-picker-list">
          {visibleItems.map(({ index, item, offset }) => {
            const renderOffset = offset - (dragOffset / scrollPickerItemHeight);
            return (
              <button
                key={index}
                type="button"
                className={`scroll-picker-item ${offset === 0 ? "is-selected" : ""}`}
                aria-selected={offset === 0}
                disabled={disabled}
                onClick={() => onChange(item)}
                style={{
                  ...getWheelItemStyle(renderOffset),
                  transition: dragging ? "none" : undefined,
                }}
              >
                {String(item).padStart(2, "0")}
                <span>{suffix}</span>
              </button>
            );
          })}
        </div>
      </div>

      <button
        type="button"
        className="scroll-picker-arrow is-down"
        onClick={() => updateByIndex(selectedIndex + 1)}
        disabled={disabled}
        aria-label={`增加${suffix}`}
      >
        <ChevronDown size={15} />
      </button>
    </div>
  );
}
