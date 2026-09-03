import { useContext, useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { AgentIdenticon } from "../components/AgentIdenticon";
import { AiAssist, AiAssistTrigger } from "../components/AiAssist";
import { Button } from "../components/Button";
import { ContextPicker } from "../components/ContextPicker";
import { DropdownSelect } from "../components/DropdownMenu";
import { EditPageFooter } from "../components/EditPageFooter";
import { FeedbackText } from "../components/FeedbackText";
import { FormField } from "../components/FormField";
import { MarkdownEditor } from "../components/MarkdownEditor";
import { Pill } from "../components/Pill";
import { RuleBuilder } from "../components/RuleBuilder";
import { ScheduleEditor } from "../components/ScheduleEditor";
import { SegmentedControl } from "../components/SegmentedControl";
import { SkillCard } from "../components/SkillCard";
import { Switch } from "../components/Switch";
import { type ScheduleFields, scheduleToWire } from "../components/scheduleHelpers";
import { TextInput } from "../components/TextInput";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useFeedback } from "../hooks/useFeedback";
import { contextLabel } from "../lib/contextArgs";
import type { ProviderModel } from "../lib/providerCatalog";
import { linesToRules, parseRules, serializeRules } from "../lib/rules";
import { useAgent, useAgentSkills } from "../queries/agent";
import { useProviderCatalog } from "../queries/catalog";
import { useProviders } from "../queries/provider";
import type { AgentWriteBody } from "../services/client";
import { client, errorMessage } from "../services/client";
import type { AgentArgDef, AgentDetail } from "../types";

interface AgentFormState {
  name: string;
  description: string;
  provider_id: string;
  model: string;
  manual: boolean;
  schedule: ScheduleFields;
  /** One expression per line; the central dispatcher tick evaluates these
   *  against incoming events. */
  rules: string;
  args: AgentArgDef[];
  prompt: string;
  spawn_per_variant: boolean;
  /** Opted-in optional skill slugs (hidden skills are injected by the engine). */
  skills: string[];
}

const EMPTY_SCHEDULE: ScheduleFields = {
  enabled: false,
  cron: "",
  timezone: "UTC",
  active: true,
};

function emptyFormState(): AgentFormState {
  return {
    name: "",
    description: "",
    provider_id: "",
    model: "",
    manual: true,
    schedule: EMPTY_SCHEDULE,
    rules: "",
    args: [],
    prompt: "",
    spawn_per_variant: false,
    skills: [],
  };
}

function formStateFrom(detail: AgentDetail): AgentFormState {
  return {
    name: detail.name,
    description: detail.description,
    provider_id: detail.provider_id ?? "",
    model: detail.model ?? "",
    manual: detail.manual,
    schedule: {
      enabled: detail.schedule != null,
      cron: detail.schedule?.cron ?? "",
      timezone: detail.schedule?.timezone ?? "UTC",
      active: detail.schedule?.enabled ?? true,
    },
    rules: (detail.rules ?? []).join("\n"),
    args: detail.args.map((a) => ({
      flag: a.flag,
      required: !!a.required,
      help: a.help ?? "",
    })),
    prompt: detail.prompt ?? "",
    spawn_per_variant: !!detail.spawn_per_variant,
    skills: detail.skills ?? [],
  };
}

// Handles `/agents/new` (create) and `/agents/edit/:id` (edit). Wraps the
// form with a fetch + loading/error guard.
export function AgentEditPage() {
  const ws = useContext(WorkspaceContext);
  const { id } = useParams<{ id: string }>();
  const { data: detail, error, isLoading } = useAgent(ws, id);

  if (error) {
    return (
      <main className="flex-1 flex items-center justify-center text-fg-muted">
        <div className="text-red-400">{errorMessage(error)}</div>
      </main>
    );
  }
  if (isLoading) {
    return <main className="flex-1 flex items-center justify-center text-fg-subtle">loading…</main>;
  }
  return <AgentForm detail={detail ?? null} />;
}

function AgentForm({ detail }: { detail: AgentDetail | null }) {
  const ws = useContext(WorkspaceContext);
  const navigate = useNavigate();
  const isCreate = detail === null;
  const [state, setState] = useState<AgentFormState>(() =>
    detail ? formStateFrom(detail) : emptyFormState(),
  );
  const [busy, setBusy] = useState(false);
  const [rulesMode, setRulesMode] = useState<"builder" | "raw">("builder");
  const fb = useFeedback();

  // Provider options (populate the select).
  const { data: providers = [] } = useProviders(ws);

  // Provider catalog: available models per provider type.
  const { data: catalog = [] } = useProviderCatalog();
  const providerCatalog = useMemo<Record<string, ProviderModel[]>>(
    () => Object.fromEntries(catalog.map((entry) => [entry.id, entry.models])),
    [catalog],
  );

  // Skills catalog (toggleable cards). Hidden skills render locked-on.
  const { data: skillCatalog = [] } = useAgentSkills(ws);

  // Always-on skills are consolidated into one "Agent Essentials" card; the
  // rest stay individually toggleable.
  const essentialSkills = skillCatalog.filter((s) => s.hidden);
  const optionalSkills = skillCatalog.filter((s) => !s.hidden);

  // Re-seed when a different detail arrives (workspace switch, refetch).
  useEffect(() => {
    setState(detail ? formStateFrom(detail) : emptyFormState());
  }, [detail]);

  function toggleSkill(slug: string) {
    setState((s) => ({
      ...s,
      skills: s.skills.includes(slug) ? s.skills.filter((x) => x !== slug) : [...s.skills, slug],
    }));
  }

  function patch<K extends keyof AgentFormState>(k: K, v: AgentFormState[K]) {
    setState((s) => ({ ...s, [k]: v }));
  }

  function buildBody(): AgentWriteBody {
    return {
      ...(isCreate ? { name: state.name.trim() } : {}),
      description: state.description.trim(),
      provider_id: state.provider_id.trim(),
      model: state.model.trim() || null,
      manual: state.manual,
      schedule: scheduleToWire(state.schedule),
      rules: linesToRules(state.rules),
      args: state.args
        .map((a) => ({
          flag: a.flag.trim(),
          required: !!a.required,
          ...(a.help && a.help.trim() ? { help: a.help.trim() } : {}),
        }))
        .filter((a) => a.flag),
      prompt: state.prompt,
      spawn_per_variant: state.spawn_per_variant,
      skills: state.skills,
    };
  }

  async function save() {
    setBusy(true);
    fb.clear();
    try {
      const out = isCreate
        ? await client.agent.create(ws, buildBody())
        : await client.agent.update(ws, detail!.id, buildBody());
      fb.ok("saved");
      navigate(`/agents/edit/${out.id}`);
    } catch (e: unknown) {
      fb.err(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!detail) return;
    if (!confirm(`Delete agent ${detail.name}?`)) return;
    setBusy(true);
    fb.clear();
    try {
      await client.agent.delete(ws, detail.id);
      navigate("/agents");
    } catch (e: unknown) {
      fb.err(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function run() {
    if (!detail) return;
    setBusy(true);
    fb.clear();
    try {
      const out = await client.agent.trigger(ws, detail.id);
      fb.ok(`running · pid ${out.pid}`, { clearAfter: 5000 });
    } catch (e: unknown) {
      fb.err(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const canSave =
    !busy &&
    (isCreate ? !!state.name.trim() : true) &&
    !!state.description.trim() &&
    !!state.provider_id.trim() &&
    !!state.prompt.trim() &&
    (!state.schedule.enabled || !!state.schedule.cron.trim());

  return (
    <main className="flex-1 overflow-y-auto px-4 py-8 font-inter">
      <div className="max-w-4xl mx-auto space-y-8">
        <header className="space-y-1">
          <div className="flex items-center gap-3 flex-wrap">
            {!isCreate && (
              <span className="inline-flex items-center justify-center w-9 h-9 rounded-md bg-surface-2 text-fg border border-border shrink-0">
                <AgentIdenticon id={detail!.name} size={28} />
              </span>
            )}
            <h1 className="text-lg font-semibold text-fg">
              {isCreate ? "New agent" : detail!.name}
            </h1>
            {!isCreate && !detail!.manual && <Pill>spawned-only</Pill>}
          </div>
        </header>

        {/* Identity + provider */}
        <section>
          <FormField
            label="Name"
            labelAccessory={
              isCreate ? (
                <span className="inline-flex items-center justify-center w-11 h-11 rounded-md bg-surface-2 text-fg border border-border shrink-0">
                  {state.name.trim() && <AgentIdenticon id={state.name.trim()} size={34} />}
                </span>
              ) : undefined
            }
          >
            {isCreate ? (
              <TextInput
                value={state.name}
                onChange={(e) => patch("name", e.target.value)}
                placeholder="Display name"
                padded
                className="w-full"
                autoFocus
              />
            ) : (
              <span className="text-fg">{detail!.name}</span>
            )}
          </FormField>
          <FormField label="Description">
            <TextInput
              value={state.description}
              onChange={(e) => patch("description", e.target.value)}
              placeholder="One-line summary shown in the dashboard"
              padded
              className="w-full"
            />
          </FormField>
          <FormField label="Prompt">
            <AiAssist
              value={state.prompt}
              onChange={(v) => patch("prompt", v)}
              fieldLabel="agent instructions"
            >
              <div className="bg-surface border border-border rounded focus-within:border-border-focus">
                <div className="flex justify-end border-b border-border px-1 py-0.5 empty:hidden">
                  <AiAssistTrigger />
                </div>
                <MarkdownEditor
                  value={state.prompt}
                  onChange={(v) => patch("prompt", v)}
                  onSave={() => {
                    if (canSave) void save();
                  }}
                  placeholder="Describe what the agent should do in each session"
                  fontSize="12px"
                  padding="0"
                  autoGrow
                  className="text-fg font-inter px-2 py-1.5 min-h-[14rem]"
                />
              </div>
            </AiAssist>
          </FormField>
          <FormField label={`Context (${state.args.length})`}>
            <div className="flex flex-wrap items-center gap-1">
              {state.args.map((a) => (
                <Pill key={a.flag} variant="flat">
                  {contextLabel(a.flag)}
                  <button
                    type="button"
                    onClick={() =>
                      patch(
                        "args",
                        state.args.filter((x) => x.flag !== a.flag),
                      )
                    }
                    className="text-fg-subtle hover:text-fg"
                    aria-label={`Remove ${contextLabel(a.flag)}`}
                  >
                    ×
                  </button>
                </Pill>
              ))}
              <ContextPicker
                selected={state.args.map((a) => a.flag)}
                onAdd={(flag) => patch("args", [...state.args, { flag, required: false }])}
              />
            </div>
          </FormField>
          <FormField label="Provider" description="Which configured provider this agent talks to.">
            <DropdownSelect
              variant="field"
              padded
              className="w-full"
              value={state.provider_id}
              onChange={(v) => patch("provider_id", v)}
              options={[
                { value: "", label: "Select a provider…" },
                ...providers.map((p) => ({ value: p.id, label: p.name })),
              ]}
            />
          </FormField>
          <FormField
            label="Model"
            description="Override the provider's default model. Leave as Default to inherit."
          >
            {(() => {
              const providerType =
                providers.find((p) => p.id === state.provider_id)?.type ?? "claude";
              const models = providerCatalog[providerType] ?? [];
              const custom =
                state.model && !models.some((m) => m.id === state.model)
                  ? [{ value: state.model, label: state.model }]
                  : [];
              return (
                <DropdownSelect
                  variant="field"
                  padded
                  className="w-full"
                  value={state.model}
                  onChange={(v) => patch("model", v)}
                  options={[
                    { value: "", label: "Default Model" },
                    ...custom,
                    ...models.map((m) => ({ value: m.id, label: m.label })),
                  ]}
                />
              );
            })()}
          </FormField>

          {skillCatalog.length > 0 && (
            <FormField
              label="Skills"
              description="Toggled skill prompts are prepended to the base prompt at spawn time. Agent Essentials are injected for every agent."
            >
              <div className="grid grid-cols-2 gap-2">
                {/* The always-on (hidden) skills are consolidated into a single
                  non-toggleable card. Later this card will toggle them all at once. */}
                {essentialSkills.length > 0 && (
                  <SkillCard
                    name="Agent Essentials"
                    description={`Core skills always available to every agent: ${essentialSkills
                      .map((s) => s.name)
                      .join(", ")}.`}
                    active
                    locked
                  />
                )}
                {optionalSkills.map((skill) => (
                  <SkillCard
                    key={skill.slug}
                    name={skill.name}
                    description={skill.description}
                    active={state.skills.includes(skill.slug)}
                    onToggle={() => toggleSkill(skill.slug)}
                  />
                ))}
              </div>
            </FormField>
          )}
          <FormField
            label="Manual"
            description='Show a "Run" button on the dashboard so the agent can be invoked by hand.'
            horizontal
          >
            <Switch checked={state.manual} onChange={(v) => patch("manual", v)} label="Manual" />
          </FormField>
          <FormField
            label="Per variant"
            description="Fan task-scoped events out into one spawn per variant."
            horizontal
          >
            <Switch
              checked={state.spawn_per_variant}
              onChange={(v) => patch("spawn_per_variant", v)}
              label="Per variant"
            />
          </FormField>

          <FormField
            label="Rules"
            description="Conditions in a row are AND-ed; the agent fires when any row is true. Builder and Raw edit the same rules."
            headerAccessory={
              <SegmentedControl>
                <Button
                  tone="tab"
                  active={rulesMode === "builder"}
                  onClick={() => setRulesMode("builder")}
                >
                  Builder
                </Button>
                <Button tone="tab" active={rulesMode === "raw"} onClick={() => setRulesMode("raw")}>
                  Raw
                </Button>
              </SegmentedControl>
            }
          >
            {rulesMode === "builder" ? (
              <RuleBuilder
                value={parseRules(linesToRules(state.rules))}
                onChange={(model) => patch("rules", serializeRules(model).join("\n"))}
              />
            ) : (
              <textarea
                value={state.rules}
                onChange={(e) => patch("rules", e.target.value)}
                placeholder={
                  "event.type == 'task.update' and event.data.pipeline_status == 'ready_for_research'"
                }
                className="w-full h-32 px-2 py-1.5 bg-surface border border-border rounded text-xs text-fg placeholder-fg-subtle focus:outline-none focus:border-border-focus font-mono leading-relaxed"
                spellCheck={false}
              />
            )}
          </FormField>

          <ScheduleEditor
            value={state.schedule}
            onChange={(s) => patch("schedule", s)}
            cronPlaceholder="0 9 * * 1-5"
          />
        </section>
        <EditPageFooter>
          <Button onClick={save} disabled={!canSave}>
            {busy ? "saving…" : isCreate ? "Create" : "Save"}
          </Button>
          {!isCreate && (
            <Button onClick={remove} disabled={busy}>
              Delete
            </Button>
          )}
          {!isCreate && detail!.manual && (
            <Button onClick={run} disabled={busy || !state.provider_id}>
              {busy ? "running…" : "Run"}
            </Button>
          )}
          <Button tone="link" onClick={() => navigate("/agents")} disabled={busy}>
            Cancel
          </Button>
          <FeedbackText feedback={fb.feedback} />
        </EditPageFooter>
      </div>
    </main>
  );
}
