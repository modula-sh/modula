import { useQueryClient } from "@tanstack/react-query";
import { useContext, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { AgentIdenticon } from "../components/AgentIdenticon";
import { Button } from "../components/Button";
import { EditPageFooter } from "../components/EditPageFooter";
import { FeedbackText } from "../components/FeedbackText";
import { FormField } from "../components/FormField";
import { Pill } from "../components/Pill";
import { McpSection } from "../components/provider-form/McpSection";
import { ProviderFields } from "../components/provider-form/ProviderFields";
import {
  createProvider,
  updateProvider,
  useProviderForm,
} from "../components/provider-form/useProviderForm";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useFeedback } from "../hooks/useFeedback";
import { ProviderTypeIcon } from "../lib/providerTypes";
import { providerKeys, useProvider } from "../queries/provider";
import { client, errorMessage } from "../services/client";
import type { ProviderDetail } from "../types";

export function ProviderEditPage() {
  const ws = useContext(WorkspaceContext);
  const { id } = useParams<{ id: string }>();
  const { data: detail, error, isLoading } = useProvider(ws, id);

  if (error) {
    return (
      <main className="flex-1 flex items-center justify-center text-fg-muted">
        <div className="text-red-400">{errorMessage(error)}</div>
      </main>
    );
  }
  if (isLoading) {
    return (
      <main className="flex-1 flex items-center justify-center text-fg-subtle">loading {id}…</main>
    );
  }
  return <ProviderForm detail={detail ?? null} />;
}

function ProviderForm({ detail }: { detail: ProviderDetail | null }) {
  const ws = useContext(WorkspaceContext);
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { state, patch, isCreate, valid } = useProviderForm(detail);
  const [busy, setBusy] = useState(false);
  const fb = useFeedback();

  async function save() {
    setBusy(true);
    fb.clear();
    try {
      if (isCreate) {
        const out = await createProvider(ws, state, true);
        queryClient.invalidateQueries({ queryKey: providerKeys.all(ws) });
        navigate(`/providers/edit/${out.id}`);
      } else {
        await updateProvider(ws, detail!.id, state, true);
        queryClient.invalidateQueries({ queryKey: providerKeys.detail(ws, detail!.id) });
        queryClient.invalidateQueries({ queryKey: providerKeys.all(ws) });
        fb.ok("saved", { clearAfter: 4000 });
      }
    } catch (e: unknown) {
      fb.err(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!detail) return;
    if (!confirm(`Delete provider ${detail.name}?`)) return;
    setBusy(true);
    fb.clear();
    try {
      await client.provider.delete(ws, detail.id);
      navigate("/providers");
    } catch (e: unknown) {
      fb.err(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const canSave = !busy && valid;

  return (
    <main className="flex-1 overflow-y-auto px-4 py-8 font-inter">
      <div className="max-w-4xl mx-auto space-y-8">
        <header className="space-y-1">
          <div className="flex items-center gap-2 flex-wrap">
            <ProviderTypeIcon type={state.type} />
            <h1 className="text-lg font-semibold text-fg">
              {isCreate ? "New provider" : detail!.name}
            </h1>
            {detail && !detail.config_dir_exists && <Pill tone="red">config dir missing</Pill>}
          </div>
        </header>

        <section>
          <ProviderFields state={state} onChange={patch} autoFocus={isCreate} />
          <McpSection rows={state.mcp_servers} onChange={(r) => patch("mcp_servers", r)} />
          {detail && <ExternalServersNote detail={detail} />}
          {detail && <AgentsUsingSection agents={detail.agents_using} />}
        </section>

        <EditPageFooter>
          <Button onClick={save} disabled={!canSave}>
            {busy ? "saving…" : isCreate ? "Create" : "Save"}
          </Button>
          {!isCreate && (
            <Button
              onClick={remove}
              disabled={busy || (detail?.agents_using.length ?? 0) > 0}
              title={
                (detail?.agents_using.length ?? 0) > 0
                  ? `${detail!.agents_using.length} agent(s) reference this provider`
                  : "Delete provider"
              }
            >
              Delete
            </Button>
          )}
          <Button tone="link" onClick={() => navigate("/providers")} disabled={busy}>
            Cancel
          </Button>
          <FeedbackText feedback={fb.feedback} />
        </EditPageFooter>
      </div>
    </main>
  );
}

function AgentsUsingSection({ agents }: { agents: string[] }) {
  return (
    <FormField label={`Agents using (${agents.length})`}>
      {agents.length === 0 ? (
        <div className="text-xs text-fg-subtle italic">none</div>
      ) : (
        <ul className="flex flex-wrap items-center gap-x-4 gap-y-2">
          {agents.map((a) => (
            <li key={a} className="inline-flex items-center gap-2">
              <span className="inline-flex items-center justify-center w-7 h-7 rounded-md bg-surface-2 text-fg border border-border shrink-0">
                <AgentIdenticon id={a} size={21} />
              </span>
              <span className="text-xs text-fg">{a}</span>
            </li>
          ))}
        </ul>
      )}
    </FormField>
  );
}

/** Project-scoped and command/stdio MCP servers the app doesn't manage (added
 * outside modula). Shown read-only so they aren't mistaken for missing. */
function ExternalServersNote({ detail }: { detail: ProviderDetail }) {
  const managed = new Set(detail.mcp_servers.map((m) => `${m.key}:${m.url}`));
  const servers = detail.projects.flatMap((p) =>
    p.mcp_servers
      .filter((s) => !(s.url && managed.has(`${s.name}:${s.url}`)))
      .map((s) => ({
        project: p.path,
        name: s.name,
        target: s.url ?? s.command ?? "-",
      })),
  );
  if (servers.length === 0) return null;

  return (
    <FormField
      label="External MCP servers"
      description="Project-scoped or command-based servers configured outside modula. Read-only here."
    >
      <ul className="space-y-1">
        {servers.map((s) => (
          <li
            key={`${s.project}:${s.name}`}
            className="flex items-center gap-2 text-[11px] text-fg-muted"
          >
            <span className="text-fg">{s.name}</span>
            <span className="font-mono truncate" title={s.target}>
              {s.target}
            </span>
          </li>
        ))}
      </ul>
    </FormField>
  );
}
