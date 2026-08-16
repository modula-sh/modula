import { defaultModelFor, type Frequency, type ScheduleModel } from "../lib/cronSchedule";

// Non-render helpers shared by ScheduleEditor and ScheduleBuilder.

/** State shape used by the schedule sub-form. */
export interface ScheduleFields {
  enabled: boolean; // "has a schedule at all" — top-level toggle
  cron: string;
  timezone: string;
  active: boolean; // schedule.enabled — only meaningful when `enabled`
}

/** Serialise schedule state to the wire shape the backend expects.
 * Returns `null` when not enabled — agents without a schedule pass
 * `schedule: null` in their POST/PUT body. */
export function scheduleToWire(s: ScheduleFields): {
  cron: string;
  timezone: string;
  enabled: boolean;
} | null {
  if (!s.enabled) return null;
  return {
    cron: s.cron.trim(),
    timezone: s.timezone.trim() || "UTC",
    enabled: s.active,
  };
}

export const FREQUENCIES: Frequency[] = ["minute", "hour", "day", "week", "month", "year"];
export const MINUTE_STEPS = [1, 2, 3, 4, 5, 6, 10, 12, 15, 20, 30];
export const HOUR_STEPS = [1, 2, 3, 4, 6, 8, 12];

export const pad2 = (n: number) => String(n).padStart(2, "0");

export function range(from: number, to: number, step = 1): number[] {
  const out: number[] = [];
  for (let n = from; n <= to; n += step) out.push(n);
  return out;
}

export function numOptions(values: number[], label: (n: number) => string = String) {
  return values.map((n) => ({ value: String(n), label: label(n) }));
}

// Preset steps plus the current value, so an off-list cron (e.g. */7) still
// shows its real selection.
export function withCurrent(presets: number[], current: number): number[] {
  return [...new Set([...presets, current])].sort((a, b) => a - b);
}

// Keep the selected time when jumping between frequencies that share it.
export function switchFrequency(cur: ScheduleModel, freq: Frequency): ScheduleModel {
  if (freq === cur.freq) return cur;
  const next = defaultModelFor(freq);
  if ("minute" in cur && "minute" in next) next.minute = cur.minute;
  if ("hour" in cur && "hour" in next) next.hour = cur.hour;
  return next;
}

export function toggleDay(days: number[], day: number): number[] {
  if (days.includes(day)) {
    const next = days.filter((d) => d !== day);
    return next.length > 0 ? next : days; // at least one weekday stays selected
  }
  return [...days, day].sort((a, b) => a - b);
}

// V8 omits "UTC" from supportedValuesOf; pin it so the default zone is always selectable.
const TIMEZONES = ["UTC", ...Intl.supportedValuesOf("timeZone").filter((z) => z !== "UTC")];

// Runtime IANA list plus the current value, so a stored zone the runtime
// doesn't enumerate still shows its real selection.
export function timezoneOptions(current: string) {
  const zones = current && !TIMEZONES.includes(current) ? [current, ...TIMEZONES] : TIMEZONES;
  return zones.map((z) => ({ value: z, label: z }));
}
