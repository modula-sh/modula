import { useEffect, useState } from "react";
import { buildCron, defaultModelFor, parseCron } from "../lib/cronSchedule";
import { Button } from "./Button";
import { DropdownSelect } from "./DropdownMenu";
import { FormField } from "./FormField";
import { ScheduleBuilder } from "./ScheduleBuilder";
import { SegmentedControl } from "./SegmentedControl";
import { Switch } from "./Switch";
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
    <>
      <FormField
        label="Schedule"
        description="Run this agent automatically on a cron schedule."
        horizontal
      >
        <Switch
          checked={value.enabled}
          onChange={(v) => patch("enabled", v)}
          label="Has a schedule"
        />
      </FormField>
      {value.enabled && (
        <>
          <FormField
            label="Cron"
            description={
              mode === "builder"
                ? "Builder and Raw edit the same cron expression."
                : "Standard 5-field cron expression."
            }
            headerAccessory={
              <SegmentedControl>
                <Button tone="tab" active={mode === "builder"} onClick={showBuilder}>
                  Builder
                </Button>
                <Button tone="tab" active={mode === "raw"} onClick={() => setMode("raw")}>
                  Raw
                </Button>
              </SegmentedControl>
            }
          >
            {mode === "builder" ? (
              <ScheduleBuilder value={model} onChange={(next) => patch("cron", buildCron(next))} />
            ) : (
              <TextInput
                value={value.cron}
                onChange={(e) => patch("cron", e.target.value)}
                placeholder={cronPlaceholder}
                mono
                padded
                className="w-full"
              />
            )}
          </FormField>
          <FormField label="Timezone" description="IANA timezone identifier.">
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
          </FormField>
          <FormField
            label="Active"
            description="When on, the cron fires while the backend is running."
            horizontal
          >
            <Switch checked={value.active} onChange={(v) => patch("active", v)} label="Active" />
          </FormField>
        </>
      )}
    </>
  );
}
