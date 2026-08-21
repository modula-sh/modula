import { useMutation } from "@tanstack/react-query";
import { MoreHorizontal } from "lucide-react";
import { DateTime } from "luxon";
import { useContext, useMemo, useState } from "react";
import { useSnapshot } from "../contexts/SnapshotContext";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useFeedback } from "../hooks/useFeedback";
import { ProviderTypeIcon } from "../lib/providerTypes";
import { client, errorMessage } from "../services/client";
import type { AgentConfig } from "../types";
import { AgentIdenticon } from "./AgentIdenticon";
import { DropdownMenu } from "./DropdownMenu";
import { FeedbackText } from "./FeedbackText";
import { Pill } from "./Pill";
import { RunAgentModal } from "./RunAgentModal";

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
  onOpen,
}: {
  agent: AgentConfig;
  isRunning: boolean;
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
        className="group flex flex-col gap-2.5 h-full border border-card-border/50 rounded-xl p-3 cursor-pointer bg-card hover:bg-surface/40 transition-colors"
      >
        <div className="flex items-center gap-2">
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

          {agent.provider_id ? (
            <ProviderTypeIcon
              type={provider?.type}
              size="sm"
              title={provider?.name ?? agent.provider_id}
            />
          ) : (
            <Pill size="sm" tone="red">
              not configured
            </Pill>
          )}

          {agent.manual && (
            <div className="ml-auto flex flex-col items-end gap-1.5 shrink-0">
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

        <div className="flex items-center gap-2 flex-wrap">
          <span className="font-inter text-fg min-w-0 truncate">{agent.name}</span>
          {!agent.manual && <Pill size="sm">spawned-only</Pill>}
          {agent.provider_id && agent.model && (
            <span className="inline-flex items-center px-2 py-0 text-[10px] rounded-full border bg-surface-2 text-fg border-border whitespace-nowrap shrink-0">
              {agent.model}
            </span>
          )}
        </div>

        {agent.description && (
          <p className="font-inter text-xs text-fg-muted line-clamp-2">{agent.description}</p>
        )}

        {agent.schedule && (
          <div className="text-[11px]">
            {agent.schedule.enabled ? (
              <div className="flex flex-col gap-0.5">
                <span>
                  <span className="font-mono text-fg">{agent.schedule.cron}</span>
                  <span className="text-fg-subtle ml-2">({agent.schedule.timezone ?? "UTC"})</span>
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
