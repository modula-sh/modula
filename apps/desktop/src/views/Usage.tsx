import { useContext, useMemo } from "react";
import { Pill } from "../components/Pill";
import { Spinner } from "../components/Spinner";
import { TabsNav } from "../components/TabsNav";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useUsage } from "../queries/usage";
import type { UsageRecord } from "../types";

/** Cost + token tracking, one row per finished claude agent run.
 *
 * Read-only: the source-of-truth data lives in claude's `result` event
 * inside each log file; this view extracts cost/tokens and aggregates by
 * agent. Click-through to logs is intentionally NOT wired — that belongs
 * on the Logs tab. */
export function UsageView() {
  const ws = useContext(WorkspaceContext);
  const { data, isPending } = useUsage(ws);
  const records: UsageRecord[] | null = isPending ? null : (data ?? []);

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <TabsNav />
      <main className="flex-1 grid grid-rows-[minmax(0,1fr)_minmax(0,2fr)] gap-3 p-3 overflow-hidden">
        <div className="grid grid-cols-2 gap-3 min-h-0">
          <WorkspaceCard records={records} />
          <AgentsCard records={records} />
        </div>
        <RunList records={records} />
      </main>
    </div>
  );
}

function RunList({ records }: { records: UsageRecord[] | null }) {
  return (
    <section className="flex flex-col overflow-hidden border border-border rounded">
      <div className="h-10 shrink-0 flex items-center gap-2 px-3 border-b border-border">
        <span className="text-fg-muted text-xs uppercase tracking-wide">Runs</span>
        <span className="text-fg-subtle text-xs">({records?.length ?? "…"})</span>
      </div>
      <div className="divide-y divide-border overflow-y-auto flex-1">
        {records === null && (
          <div className="px-3 py-4">
            <Spinner />
          </div>
        )}
        {records && records.length === 0 && (
          <div className="px-3 py-4 text-fg-subtle text-xs">no completed agent runs yet</div>
        )}
        {records?.map((r) => (
          <article key={r.log} className="px-3 py-2 flex items-center gap-3" title={r.log}>
            <Pill>{r.agent}</Pill>
            <span className="text-[11px] text-fg-subtle font-mono">{r.mtime}</span>
            <span className="text-[11px] text-fg-subtle font-mono">
              {(r.duration_ms / 1000).toFixed(1)}s
            </span>
            <span className="text-[11px] text-fg-subtle font-mono">in {fmt(r.tokens.input)}</span>
            <span className="text-[11px] text-fg-subtle font-mono">out {fmt(r.tokens.output)}</span>
            <span className="text-[11px] text-fg-subtle font-mono">
              cache-w {fmt(r.tokens.cache_creation)}
            </span>
            <span className="text-[11px] text-fg-subtle font-mono">
              cache-r {fmt(r.tokens.cache_read)}
            </span>
            <span className="ml-auto text-xs text-fg font-mono shrink-0">
              {formatCost(r.cost_usd)}
            </span>
          </article>
        ))}
      </div>
    </section>
  );
}

function WorkspaceCard({ records }: { records: UsageRecord[] | null }) {
  const stats = useMemo(() => aggregate(records ?? []), [records]);
  const loading = records === null;

  return (
    <section className="flex flex-col overflow-hidden border border-border rounded">
      <div className="h-10 shrink-0 flex items-center gap-2 px-3 border-b border-border">
        <span className="text-fg-muted text-xs uppercase tracking-wide">Workspace</span>
      </div>
      <div className="overflow-y-auto flex-1 px-3 py-3">
        {loading ? (
          <Spinner />
        ) : (
          <>
            <dl className="grid grid-cols-[max-content_1fr] gap-x-3 gap-y-0.5 text-[11px]">
              <dt className="text-fg-subtle">cost</dt>
              <dd className="text-fg font-mono">{formatCost(stats.cost)}</dd>
              <dt className="text-fg-subtle">runs</dt>
              <dd className="text-fg font-mono">{stats.runs}</dd>
            </dl>
            <div className="mt-3 space-y-1.5">
              <TokenBar label="input" value={stats.tokens.input} max={stats.tokensMax} />
              <TokenBar label="output" value={stats.tokens.output} max={stats.tokensMax} />
              <TokenBar label="cache-w" value={stats.tokens.cache_creation} max={stats.tokensMax} />
              <TokenBar label="cache-r" value={stats.tokens.cache_read} max={stats.tokensMax} />
            </div>
          </>
        )}
      </div>
    </section>
  );
}

function AgentsCard({ records }: { records: UsageRecord[] | null }) {
  const stats = useMemo(() => aggregate(records ?? []), [records]);
  const loading = records === null;
  const maxCost = stats.byAgent[0]?.cost ?? 0;

  return (
    <section className="flex flex-col overflow-hidden border border-border rounded">
      <div className="h-10 shrink-0 flex items-center gap-2 px-3 border-b border-border">
        <span className="text-fg-muted text-xs uppercase tracking-wide">By agent</span>
      </div>
      <div className="overflow-y-auto flex-1 px-3 py-3">
        {loading ? (
          <Spinner />
        ) : stats.byAgent.length === 0 ? (
          <div className="text-fg-subtle text-xs">no agent runs yet</div>
        ) : (
          <ul className="space-y-2.5">
            {stats.byAgent.map((row) => (
              <li key={row.agent}>
                <div className="flex items-baseline gap-2">
                  <Pill>{row.agent}</Pill>
                  <span className="text-[11px] text-fg-subtle">
                    {row.runs} run{row.runs === 1 ? "" : "s"}
                  </span>
                  <span className="ml-auto text-[11px] text-fg font-mono">
                    {formatCost(row.cost)}
                  </span>
                </div>
                <Bar value={row.cost} max={maxCost} className="mt-2" />
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

function TokenBar({ label, value, max }: { label: string; value: number; max: number }) {
  return (
    <div>
      <div className="flex items-baseline justify-between text-[11px]">
        <span className="text-fg-subtle">{label}</span>
        <span className="text-fg font-mono">{fmt(value)}</span>
      </div>
      <Bar value={value} max={max} className="mt-1" />
    </div>
  );
}

function Bar({ value, max, className = "" }: { value: number; max: number; className?: string }) {
  const pct = max > 0 ? Math.max(1, Math.round((value / max) * 100)) : 0;
  return (
    <div className={`h-1 bg-surface-2 rounded overflow-hidden ${className}`.trim()}>
      <div className="h-full bg-fg-muted" style={{ width: `${pct}%` }} aria-hidden />
    </div>
  );
}

interface AgentStats {
  agent: string;
  cost: number;
  runs: number;
}

interface AggregatedStats {
  cost: number;
  runs: number;
  tokens: {
    input: number;
    output: number;
    cache_creation: number;
    cache_read: number;
  };
  tokensMax: number;
  byAgent: AgentStats[];
}

function aggregate(records: UsageRecord[]): AggregatedStats {
  const tokens = { input: 0, output: 0, cache_creation: 0, cache_read: 0 };
  let cost = 0;
  const per = new Map<string, AgentStats>();
  for (const r of records) {
    cost += r.cost_usd;
    tokens.input += r.tokens.input;
    tokens.output += r.tokens.output;
    tokens.cache_creation += r.tokens.cache_creation;
    tokens.cache_read += r.tokens.cache_read;
    const cur = per.get(r.agent) ?? { agent: r.agent, cost: 0, runs: 0 };
    cur.cost += r.cost_usd;
    cur.runs += 1;
    per.set(r.agent, cur);
  }
  const tokensMax = Math.max(tokens.input, tokens.output, tokens.cache_creation, tokens.cache_read);
  const byAgent = Array.from(per.values()).sort((a, b) => b.cost - a.cost);
  return { cost, runs: records.length, tokens, tokensMax, byAgent };
}

/** Compact integer: `12,345` → `12.3k`, `1,234,567` → `1.23M`. */
function fmt(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(1)}k`;
  return `${(n / 1_000_000).toFixed(2)}M`;
}

/** USD cost. Sub-cent values show 4 decimal places so they don't collapse
 * to `$0.00`; otherwise 2 decimals. */
function formatCost(usd: number): string {
  if (usd === 0) return "$0.00";
  return usd < 0.01 ? `$${usd.toFixed(4)}` : `$${usd.toFixed(2)}`;
}
