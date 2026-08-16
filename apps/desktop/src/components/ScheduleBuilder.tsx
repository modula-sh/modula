import {
  type Frequency,
  MONTH_LABELS,
  type ScheduleModel,
  WEEKDAY_LABELS,
} from "../lib/cronSchedule";
import { DropdownSelect } from "./DropdownMenu";
import {
  FREQUENCIES,
  HOUR_STEPS,
  MINUTE_STEPS,
  numOptions,
  pad2,
  range,
  switchFrequency,
  toggleDay,
  withCurrent,
} from "./scheduleHelpers";

// Sentence-style schedule picker over ScheduleModel. Controlled: every edit
// emits the full next model; ScheduleEditor serialises it back to a cron.

// Matches RuleBuilder's TOKEN_BTN height so pills line up with field dropdowns.
const DAY_BTN = "h-[1.625rem] min-w-[2.25rem] px-1.5 border rounded text-[11px] leading-none";

export function ScheduleBuilder({
  value,
  onChange,
}: {
  value: ScheduleModel;
  onChange: (next: ScheduleModel) => void;
}) {
  const plural = "step" in value && value.step > 1;
  const freqOptions = FREQUENCIES.map((f) => ({
    value: f,
    label: f === value.freq && plural ? `${f}s` : f,
  }));
  return (
    <div className="flex flex-wrap items-center gap-x-1.5 gap-y-2 text-[11px] text-fg-muted">
      <span>Every</span>
      {(value.freq === "minute" || value.freq === "hour") && (
        <DropdownSelect
          variant="field"
          mono
          panelClassName="w-20"
          value={String(value.step)}
          onChange={(v) => onChange({ ...value, step: Number(v) })}
          options={numOptions(
            withCurrent(value.freq === "minute" ? MINUTE_STEPS : HOUR_STEPS, value.step),
          )}
        />
      )}
      <DropdownSelect
        variant="field"
        panelClassName="w-32"
        value={value.freq}
        onChange={(f) => onChange(switchFrequency(value, f as Frequency))}
        options={freqOptions}
      />
      {value.freq === "hour" && (
        <>
          <span>at minute</span>
          <DropdownSelect
            variant="field"
            mono
            panelClassName="w-20"
            value={String(value.minute)}
            onChange={(v) => onChange({ ...value, minute: Number(v) })}
            options={numOptions(range(0, 59), pad2)}
          />
        </>
      )}
      {value.freq === "week" && (
        <>
          <span>on</span>
          <span className="inline-flex items-center gap-1">
            {WEEKDAY_LABELS.map((label, i) => {
              const day = i + 1;
              const active = value.days.includes(day);
              return (
                <button
                  key={label}
                  type="button"
                  onClick={() => onChange({ ...value, days: toggleDay(value.days, day) })}
                  className={`${DAY_BTN} ${
                    active
                      ? "bg-surface-2 text-fg border-border-focus/40"
                      : "text-fg-subtle border-border hover:text-fg hover:bg-surface"
                  }`}
                >
                  {label}
                </button>
              );
            })}
          </span>
        </>
      )}
      {value.freq === "month" && (
        <>
          <span>on day</span>
          <DropdownSelect
            variant="field"
            mono
            panelClassName="w-20"
            value={String(value.day)}
            onChange={(v) => onChange({ ...value, day: Number(v) })}
            options={numOptions(range(1, 31))}
          />
        </>
      )}
      {value.freq === "year" && (
        <>
          <span>on</span>
          <DropdownSelect
            variant="field"
            panelClassName="w-24"
            value={String(value.month)}
            onChange={(v) => onChange({ ...value, month: Number(v) })}
            options={numOptions(range(1, 12), (n) => MONTH_LABELS[n - 1])}
          />
          <DropdownSelect
            variant="field"
            mono
            panelClassName="w-20"
            value={String(value.day)}
            onChange={(v) => onChange({ ...value, day: Number(v) })}
            options={numOptions(range(1, 31))}
          />
        </>
      )}
      {(value.freq === "day" ||
        value.freq === "week" ||
        value.freq === "month" ||
        value.freq === "year") && (
        <>
          <span>at</span>
          <DropdownSelect
            variant="field"
            mono
            panelClassName="w-20"
            value={String(value.hour)}
            onChange={(v) => onChange({ ...value, hour: Number(v) })}
            options={numOptions(range(0, 23), pad2)}
          />
          <span>:</span>
          <DropdownSelect
            variant="field"
            mono
            panelClassName="w-20"
            value={String(value.minute)}
            onChange={(v) => onChange({ ...value, minute: Number(v) })}
            options={numOptions(withCurrent(range(0, 55, 5), value.minute), pad2)}
          />
        </>
      )}
    </div>
  );
}
