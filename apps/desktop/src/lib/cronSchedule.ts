// Pure conversion between 5-field cron strings and the frequency-based model
// behind ScheduleBuilder. parseCron is best-effort: anything the model can't
// express (names, steps outside minute/hour, 6-field, …) returns null and the
// UI falls back to Raw mode. Weekdays are 1–7 with 1 = Monday, 7 = Sunday: the
// engine validates with the `cron` crate (rejects 0) but fires via croner
// (classic semantics, 7 = Sunday), and 1–7 is the vocabulary valid in both.

export type Frequency = "minute" | "hour" | "day" | "week" | "month" | "year";

export type ScheduleModel =
  | { freq: "minute"; step: number }
  | { freq: "hour"; step: number; minute: number }
  | { freq: "day"; hour: number; minute: number }
  | { freq: "week"; days: number[]; hour: number; minute: number }
  | { freq: "month"; day: number; hour: number; minute: number }
  | { freq: "year"; month: number; day: number; hour: number; minute: number };

// Index = weekday - 1 (1 = Monday … 7 = Sunday) / month - 1.
export const WEEKDAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
export const MONTH_LABELS = [
  "Jan",
  "Feb",
  "Mar",
  "Apr",
  "May",
  "Jun",
  "Jul",
  "Aug",
  "Sep",
  "Oct",
  "Nov",
  "Dec",
];

export function buildCron(m: ScheduleModel): string {
  switch (m.freq) {
    case "minute":
      return m.step === 1 ? "* * * * *" : `*/${m.step} * * * *`;
    case "hour":
      return m.step === 1 ? `${m.minute} * * * *` : `${m.minute} */${m.step} * * *`;
    case "day":
      return `${m.minute} ${m.hour} * * *`;
    case "week": {
      const days = [...new Set(m.days)].sort((a, b) => a - b);
      return `${m.minute} ${m.hour} * * ${days.length > 0 ? days.join(",") : "*"}`;
    }
    case "month":
      return `${m.minute} ${m.hour} ${m.day} * *`;
    case "year":
      return `${m.minute} ${m.hour} ${m.day} ${m.month} *`;
  }
}

function num(field: string, min: number, max: number): number | null {
  if (!/^\d+$/.test(field)) return null;
  const n = Number(field);
  return n >= min && n <= max ? n : null;
}

// "*" → 1, "*/N" → N; null for anything else.
function starStep(field: string, max: number): number | null {
  if (field === "*") return 1;
  const m = /^\*\/(\d+)$/.exec(field);
  if (!m) return null;
  const n = Number(m[1]);
  return n >= 1 && n <= max ? n : null;
}

// Numeric weekday list; simple ranges expand, 0 normalises to 7 (Sunday).
function parseWeekdays(field: string): number[] | null {
  const days: number[] = [];
  for (const part of field.split(",")) {
    const range = /^(\d+)-(\d+)$/.exec(part);
    if (range) {
      const a = Number(range[1]);
      const b = Number(range[2]);
      if (a > b || b > 7) return null;
      for (let d = a; d <= b; d += 1) days.push(d === 0 ? 7 : d);
    } else {
      const n = num(part, 0, 7);
      if (n === null) return null;
      days.push(n === 0 ? 7 : n);
    }
  }
  const uniq = [...new Set(days)].sort((a, b) => a - b);
  return uniq.length > 0 ? uniq : null;
}

export function parseCron(cron: string): ScheduleModel | null {
  const fields = cron.trim().split(/\s+/);
  if (fields.length !== 5) return null;
  const [minF, hrF, domF, monF, dowF] = fields;

  if (domF === "*" && monF === "*" && dowF === "*") {
    if (minF === "*" || minF.startsWith("*/")) {
      if (hrF !== "*") return null;
      const step = starStep(minF, 59);
      return step === null ? null : { freq: "minute", step };
    }
    const minute = num(minF, 0, 59);
    if (minute === null) return null;
    if (hrF === "*" || hrF.startsWith("*/")) {
      const step = starStep(hrF, 23);
      return step === null ? null : { freq: "hour", step, minute };
    }
    const hour = num(hrF, 0, 23);
    return hour === null ? null : { freq: "day", hour, minute };
  }

  const minute = num(minF, 0, 59);
  const hour = num(hrF, 0, 23);
  if (minute === null || hour === null) return null;

  if (domF === "*" && monF === "*") {
    const days = parseWeekdays(dowF);
    return days === null ? null : { freq: "week", days, hour, minute };
  }
  if (monF === "*" && dowF === "*") {
    const day = num(domF, 1, 31);
    return day === null ? null : { freq: "month", day, hour, minute };
  }
  if (dowF === "*") {
    const day = num(domF, 1, 31);
    const month = num(monF, 1, 12);
    if (day === null || month === null) return null;
    return { freq: "year", month, day, hour, minute };
  }
  return null;
}

export function defaultModelFor(freq: Frequency): ScheduleModel {
  switch (freq) {
    case "minute":
      return { freq, step: 15 };
    case "hour":
      return { freq, step: 1, minute: 0 };
    case "day":
      return { freq, hour: 9, minute: 0 };
    case "week":
      return { freq, days: [1], hour: 9, minute: 0 };
    case "month":
      return { freq, day: 1, hour: 9, minute: 0 };
    case "year":
      return { freq, month: 1, day: 1, hour: 9, minute: 0 };
  }
}
