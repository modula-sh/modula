import { Search } from "lucide-react";
import { useEffect, useState } from "react";
import {
  useIntegrationRepos,
  useIntegrationSearch,
  useIntegrations,
} from "../../queries/integration";
import { client, errorMessage } from "../../services/client";
import type { ExternalItem } from "../../types";
import { BaseModal } from "../BaseModal";
import { Button } from "../Button";
import { DropdownSelect } from "../DropdownMenu";
import { Pill } from "../Pill";
import { Spinner } from "../Spinner";
import { TextInput } from "../TextInput";
import { INTEGRATIONS } from "./IntegrationsList";

const DEBOUNCE_MS = 300;

/** Search a connected integration and import one external item as a task.
 * Import fetches the full item, then goes through the existing task upsert
 * path — re-importing the same key updates rather than duplicates. */
export function ImportTaskModal({
  workspace,
  onCreated,
  onCancel,
}: {
  workspace: string;
  onCreated: (newId: string) => void;
  onCancel: () => void;
}) {
  const { data: integrations } = useIntegrations(workspace);
  const connectedIds = new Set((integrations ?? []).map((i) => i.id));

  const [id, setId] = useState("");
  const [repo, setRepo] = useState("");
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const [selected, setSelected] = useState<ExternalItem | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const trimmed = query.trim();
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(trimmed), DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [trimmed]);

  const { data: repos, error: reposError } = useIntegrationRepos(workspace, id);

  // gh needs a target repository, picked here rather than stored in the config.
  const params = id === "github" ? { repo } : {};
  const ready = !!id && (id !== "github" || !!repo);
  const {
    data: results,
    isFetching,
    error: searchError,
  } = useIntegrationSearch(workspace, ready ? id : "", debounced, params);

  async function importSelected() {
    if (!selected) return;
    setBusy(true);
    setError(null);
    try {
      const item = await client.integration.fetchItem(workspace, id, selected.key, params);
      const out = await client.task.upsert(workspace, {
        source: id,
        external_id: item.key,
        title: item.title,
        description: item.description,
        url: item.url,
        source_data: { state: item.state },
      });
      onCreated(out.id);
    } catch (e: unknown) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <BaseModal open busy={busy} onCancel={onCancel} panelClassName="w-[34rem] min-h-[24rem]">
      <div className="text-base font-semibold text-fg">Import task</div>
      <div className="flex flex-col gap-2">
        <DropdownSelect
          variant="field"
          padded
          className="w-full"
          value={id}
          placeholder="Integration"
          options={INTEGRATIONS.map(({ id: value, name, Icon }) => ({
            value,
            label: name,
            icon: <Icon className="w-3.5 h-3.5" />,
            disabled: !connectedIds.has(value),
          }))}
          onChange={(v) => {
            setId(v);
            setSelected(null);
          }}
        />
        {id === "github" && (
          <DropdownSelect
            variant="field"
            padded
            mono
            className="w-full"
            panelClassName="w-72"
            value={repo}
            placeholder="Repository"
            options={(repos ?? []).map((r) => ({ value: r, label: r }))}
            onChange={(v) => {
              setRepo(v);
              setSelected(null);
            }}
          />
        )}
        <div className="relative">
          <Search
            size={13}
            className={`absolute left-2 top-1/2 -translate-y-1/2 pointer-events-none text-fg-subtle ${ready ? "" : "opacity-60"}`}
          />
          <TextInput
            padded
            className="w-full pl-7"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setSelected(null);
            }}
            placeholder="Search tickets…"
            disabled={!ready}
            autoFocus
          />
        </div>
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto">
        {connectedIds.size === 0 ? (
          <p className="px-2 py-3 text-sm text-fg-muted">
            No integrations connected. Add one in Settings.
          </p>
        ) : isFetching && !results ? (
          <div className="flex items-center gap-2 px-2 py-3 text-fg-muted text-xs">
            <Spinner size={12} /> searching…
          </div>
        ) : (
          (results ?? []).map((item) => (
            <button
              key={item.key}
              type="button"
              onClick={() => setSelected(selected?.key === item.key ? null : item)}
              className={`grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-2 w-full text-left px-2 py-1.5 rounded transition-colors ${selected?.key === item.key ? "bg-surface-2 hover:bg-border" : "hover:bg-surface/50"}`}
            >
              <span className="font-mono text-[11px] text-fg-subtle">{item.key}</span>
              <span className="font-inter text-[13px] text-fg truncate">{item.title}</span>
              {item.state && (
                <Pill size="sm" variant="flat">
                  {item.state}
                </Pill>
              )}
            </button>
          ))
        )}
        {results?.length === 0 && (
          <div className="px-2 py-3 text-fg-subtle text-xs">no results</div>
        )}
      </div>
      <div className="flex items-center gap-2 mt-auto">
        <span className="ml-auto flex items-center gap-2">
          <Button
            onClick={onCancel}
            disabled={busy}
            tone="link"
            className="px-2 py-1 rounded transition-colors enabled:hover:bg-surface"
          >
            Cancel
          </Button>
          <Button onClick={importSelected} disabled={busy || !selected}>
            {busy ? "importing…" : "Import"}
          </Button>
        </span>
      </div>
      {(error || searchError || reposError) && (
        <div className="text-[11px] text-red-400">
          {error ?? errorMessage(searchError ?? reposError)}
        </div>
      )}
    </BaseModal>
  );
}
