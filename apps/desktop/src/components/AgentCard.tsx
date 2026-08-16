import { useMutation } from "@tanstack/react-query";
import { MoreHorizontal } from "lucide-react";
import { DateTime } from "luxon";
import { Fragment, type ReactNode, useContext, useMemo, useState } from "react";
import { useSnapshot } from "../contexts/SnapshotContext";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useFeedback } from "../hooks/useFeedback";
import { ProviderTypeIcon } from "../lib/providerTypes";
import { parseRules, type RuleComparison } from "../lib/rules";
import { optLabel, ruleKeyFor } from "../lib/rulesSchema";
import { client, errorMessage } from "../services/client";
import type { AgentConfig } from "../types";
import { AgentIdenticon } from "./AgentIdenticon";
import { DropdownMenu } from "./DropdownMenu";
import { FeedbackText } from "./FeedbackText";
import { Pill } from "./Pill";
import { RunAgentModal } from "./RunAgentModal";
import { TimeAgo } from "./TimeAgo";

// Tidy label for a comparison's value: enum/bool get the lowercase token label
// (matching the builder); freeform ids stay verbatim so digits aren't stripped.
function ruleValueLabel(c: RuleComparison): string {
  const key = ruleKeyFor(c.key);
  return key && key.valueKind !== "freeform" ? optLabel(c.value) : c.value;
}

// One token block, shared by the event type, every condition key/op/value, and
// the raw fallback (which passes `className` to allow wrapping long text).
function Chip({ children, className = "" }: { children: ReactNode; className?: string }) {
  return (
    <span
      className={`inline-flex items-center rounded-full bg-surface px-2 py-0.5 font-mono text-[10px] text-fg ${className}`.trim()}
    >
      {children}
    </span>
  );
}

// Read-only token rendering of one rule. Reuses the builder's parser + labels so
// cards and the editor speak the same language; an expression the builder can't
// reduce (OR, parens, …) falls back to a single chip holding the raw text.
function RuleTokens({ rule }: { rule: string }) {
  const row = parseRules([rule])[0];
  if (!row || row.raw != null) {
    return (
      <div className="flex min-w-0" title={rule}>
        <Chip className="min-w-0 break-all">{rule}</Chip>
      </div>
    );
  }

  const trigger = row.comparisons.find((c) => c.key === "event.type" && c.op === "==");
  const conditions = row.comparisons.filter((c) => c !== trigger);

  return (
    <div className="flex flex-wrap items-center gap-1 min-w-0" title={rule}>
      {trigger && <Chip>{optLabel(trigger.value)}</Chip>}
      {conditions.map((c, i) => (
        <Fragment key={i}>
          {(trigger || i > 0) && (
            <span className="text-[9px] uppercase tracking-wide text-fg-subtle">and</span>
          )}
          <Chip>{ruleKeyFor(c.key)?.label ?? c.key}</Chip>
          <Chip>{c.op === "!=" ? "≠" : "="}</Chip>
          <Chip>{ruleValueLabel(c)}</Chip>
        </Fragment>
      ))}
    </div>
  );
}

function formatNextFire(iso: string): string {
  const dt = DateTime.fromISO(iso);
  if (!dt.isValid) return iso;
  const now = DateTime.now();
  const time = dt.toFormat("h:mm a");
  if (dt.hasSame(now, "day")) return `at ${time}`;
  if (dt.hasSame(now.plus({ days: 1 }), "day")) return `tomorrow at ${time}`;
  if (dt.diff(now, "days").days < 7) return `${dt.toFormat("EEE")} at ${time}`;
  return `${dt.toFormat("LLL d")} at ${time}`;
}

export function AgentCard({
  agent,
  isRunning,
  lastLog,
  onOpen,
}: {
  agent: AgentConfig;
  isRunning: boolean;
  lastLog: string | null;
  onOpen: () => void;
}) {
  const ws = useContext(WorkspaceContext);
  const { snap } = useSnapshot();
  const provider = useMemo(() => {
    if (!agent.provider_id) return null;
    return snap?.config.providers.find((p) => p.id === agent.provider_id) ?? null;
  }, [snap, agent.provider_id]);
  const [modalOpen, setModalOpen] = useState(false);
  const fb = useFeedback();

  // Snapshot-owned: the run reflects via the SSE snapshot, so nothing to invalidate.
  const run = useMutation({
    mutationFn: (args: Record<string, string>) => client.agent.trigger(ws, agent.id, args),
    onSuccess: (data) => {
      fb.ok(`running · pid ${data.pid}`, { clearAfter: 5000 });
      setModalOpen(false);
    },
    onError: (e) => fb.err(errorMessage(e), { clearAfter: 5000 }),
  });
  const busy = run.isPending;

  function trigger(args: Record<string, string>) {
    fb.clear();
    run.mutate(args);
  }

  return (
    <>
      <article
        onClick={onOpen}
        className="group flex flex-col h-full border border-card-border/50 rounded-xl p-3 cursor-pointer bg-card hover:bg-surface/40 transition-colors"
      >
        <div className="flex items-center gap-3">
          <span
            className="relative inline-flex items-center justify-center w-9 h-9 rounded-md bg-surface-2 text-fg border border-border shrink-0"
            aria-hidden
          >
            <AgentIdenticon id={agent.name} size={28} />
            {isRunning && (
              <span
                className="absolute -top-0.5 -right-0.5 w-2 h-2 rounded-full ring-2 ring-bg bg-green-500 backdrop-blur-sm animate-pulse"
                title="running"
              />
            )}
          </span>

          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 flex-wrap">
              <span className="font-inter font-medium text-fg truncate">{agent.name}</span>
              {!agent.manual && <Pill size="sm">spawned-only</Pill>}
              {agent.provider_id ? (
                <span className="inline-flex items-center gap-1.5 text-xs text-fg-muted ml-1.5">
                  <ProviderTypeIcon type={provider?.type} size="xs" />
                  <span>{provider?.name ?? agent.provider_id}</span>
                </span>
              ) : (
                <span className="ml-1.5 inline-flex">
                  <Pill size="sm" tone="red">
                    not configured
                  </Pill>
                </span>
              )}
              {agent.provider_id && agent.model && (
                <span className="inline-flex items-center px-2 py-0 text-[10px] rounded-full border bg-surface-2 text-fg border-border whitespace-nowrap shrink-0">
                  {agent.model}
                </span>
              )}
            </div>
          </div>

          {agent.manual && (
            <div className="flex flex-col items-end gap-1.5 shrink-0">
              <DropdownMenu
                panelClassName="w-40"
                trigger={({ open, toggle }) => (
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      toggle();
                    }}
                    title="More actions"
                    className={
                      "p-1.5 rounded text-fg-subtle hover:text-fg hover:bg-surface-2 transition-colors " +
                      (open ? "bg-surface-2 text-fg" : "")
                    }
                  >
                    <MoreHorizontal size={16} />
                  </button>
                )}
              >
                {({ close }) => (
                  <ul className="space-y-0.5" onClick={(e) => e.stopPropagation()}>
                    <li>
                      <button
                        type="button"
                        onClick={() => {
                          close();
                          setModalOpen(true);
                        }}
                        disabled={busy || !agent.provider_id}
                        title={
                          !agent.provider_id ? "agent has no provider_id, assign one to enable" : ""
                        }
                        className="w-full text-left px-2 py-1.5 rounded text-xs text-fg-muted hover:bg-surface hover:text-fg disabled:opacity-50 disabled:cursor-not-allowed"
                      >
                        {busy ? "spawning…" : "Run"}
                      </button>
                    </li>
                  </ul>
                )}
              </DropdownMenu>
              {!modalOpen && <FeedbackText feedback={fb.feedback} />}
            </div>
          )}
        </div>

        {agent.description && (
          <p className="font-inter text-xs text-fg-muted mt-3 line-clamp-2">{agent.description}</p>
        )}

        <div className="mt-3 flex flex-col gap-1.5 text-[11px]">
          {agent.schedule && (
            <div>
              {agent.schedule.enabled ? (
                <div className="flex flex-col gap-0.5">
                  <span>
                    <span className="font-mono text-fg">{agent.schedule.cron}</span>
                    <span className="text-fg-subtle ml-2">
                      ({agent.schedule.timezone ?? "UTC"})
                    </span>
                  </span>
                  {agent.next_fire && (
                    <span
                      className="text-fg-subtle"
                      title={DateTime.fromISO(agent.next_fire).toFormat("LLL d, yyyy h:mm a ZZZZ")}
                    >
                      next {formatNextFire(agent.next_fire)}
                    </span>
                  )}
                </div>
              ) : (
                <span className="text-fg-subtle">disabled in config</span>
              )}
            </div>
          )}

          {agent.rules.length > 0 && (
            <div className="flex flex-col gap-1.5">
              {agent.rules.map((r, i) => (
                <RuleTokens key={i} rule={r} />
              ))}
            </div>
          )}
        </div>

        {lastLog && (
          <div className="mt-auto pt-3 text-[11px]">
            <TimeAgo iso={lastLog} className="text-fg-subtle" />
          </div>
        )}
      </article>
      <RunAgentModal
        open={modalOpen}
        agent={agent}
        busy={busy}
        feedback={fb.feedback}
        onRun={trigger}
        onCancel={() => setModalOpen(false)}
      />
    </>
  );
}
