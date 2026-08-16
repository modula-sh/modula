import { useEffect, useState } from "react";
import { buildCron, defaultModelFor, parseCron } from "../lib/cronSchedule";
import { Button } from "./Button";
import { DropdownSelect } from "./DropdownMenu";
import { FieldRow } from "./FieldRow";
import { ScheduleBuilder } from "./ScheduleBuilder";
import { SectionLabel } from "./SectionLabel";
import type { ScheduleFields } from "./scheduleHelpers";
import { timezoneOptions } from "./scheduleHelpers";
import { TextInput } from "./TextInput";

/** Schedule editor — has-a-schedule toggle + builder/raw cron, timezone, active. */
export function ScheduleEditor({
  value,
  onChange,
  cronPlaceholder = "0 9 * * 1-5",
}: {
  value: ScheduleFields;
  onChange: (next: ScheduleFields) => void;
  cronPlaceholder?: string;
}) {
  // Raw only when an existing cron can't be expressed by the builder.
  const [mode, setMode] = useState<"builder" | "raw">(() =>
    value.cron.trim() !== "" && parseCron(value.cron) === null ? "raw" : "builder",
  );
  const parsed = parseCron(value.cron);
  const model = parsed ?? defaultModelFor("day");

  function patch<K extends keyof ScheduleFields>(k: K, v: ScheduleFields[K]) {
    onChange({ ...value, [k]: v });
  }

  // Builder mode always reflects a concrete cron; seed one when none exists.
  // A re-seeded form (detail refetch) can hand Builder an unrepresentable
  // cron — fall back to Raw rather than misrepresent it.
  const seedCron = value.enabled && mode === "builder" && value.cron.trim() === "";
  const stranded = mode === "builder" && value.cron.trim() !== "" && parsed === null;
  useEffect(() => {
    if (seedCron) patch("cron", buildCron(defaultModelFor("day")));
    if (stranded) setMode("raw");
  });

  // Switching to Builder with an unrepresentable cron replaces it — the user
  // acted explicitly, and Raw remains the escape hatch.
  function showBuilder() {
    if (value.cron.trim() !== "" && parseCron(value.cron) === null) {
      patch("cron", buildCron(defaultModelFor("day")));
    }
    setMode("builder");
  }

  return (
    <section className="border border-card-border/50 bg-card rounded-xl p-3 space-y-2.5">
      <div className="flex items-start justify-between gap-4">
        <SectionLabel className="min-w-0 flex-1">schedule</SectionLabel>
        {value.enabled && (
          <div className="flex items-center gap-1 shrink-0">
            <Button tone="tab" active={mode === "builder"} onClick={showBuilder}>
              Builder
            </Button>
            <Button tone="tab" active={mode === "raw"} onClick={() => setMode("raw")}>
              Raw
            </Button>
          </div>
        )}
      </div>
      <label className="flex items-center gap-2 text-[11px] text-fg">
        <input
          type="checkbox"
          checked={value.enabled}
          onChange={(e) => patch("enabled", e.target.checked)}
        />
        <span>Has a schedule</span>
      </label>
      {value.enabled && (
        <>
          {mode === "builder" ? (
            <ScheduleBuilder value={model} onChange={(next) => patch("cron", buildCron(next))} />
          ) : (
            <FieldRow label="cron" description="Standard 5-field cron expression.">
              <TextInput
                value={value.cron}
                onChange={(e) => patch("cron", e.target.value)}
                placeholder={cronPlaceholder}
                mono
                padded
                className="w-full"
              />
            </FieldRow>
          )}
          <FieldRow label="timezone" description="IANA timezone identifier.">
            <DropdownSelect
              variant="field"
              mono
              padded
              className="w-full"
              panelClassName="w-64"
              value={value.timezone.trim() || "UTC"}
              onChange={(v) => patch("timezone", v)}
              options={timezoneOptions(value.timezone.trim() || "UTC")}
            />
          </FieldRow>
          <FieldRow
            label="enabled"
            description="When checked, the cron fires while the backend is running."
          >
            <input
              type="checkbox"
              checked={value.active}
              onChange={(e) => patch("active", e.target.checked)}
            />
          </FieldRow>
        </>
      )}
    </section>
  );
}
