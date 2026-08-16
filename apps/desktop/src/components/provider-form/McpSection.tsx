import { ChevronDown, ChevronRight, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import {
  IntegrationIcon,
  MCP_PRESETS,
  type McpPreset,
  matchIntegrations,
} from "../../lib/mcpIntegrations";
import { DropdownMenu } from "../DropdownMenu";
import { IconButton } from "../IconButton";
import { GenericProviderIcon } from "../icons/GenericProviderIcon";
import { SectionLabel } from "../SectionLabel";
import { TextInput } from "../TextInput";
import type { McpRowState } from "./ProviderFields";
import { nextRid } from "./useProviderForm";

/** Accordion MCP editor: one row per managed HTTP server, an "Add MCP" picker
 * with Jira/Linear/GitHub presets and an Other path. The row list is
 * authoritative — removing a row and saving deletes it from the config. */
export function McpSection({
  rows,
  onChange,
}: {
  rows: McpRowState[];
  onChange: (rows: McpRowState[]) => void;
}) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  function add(row: McpRowState) {
    onChange([...rows, row]);
    setExpanded((s) => new Set(s).add(row.rid));
  }
  function addPreset(p: McpPreset) {
    add({ rid: nextRid(), key: p.key, url: p.url, auth_token: "", preset: true });
  }
  function addOther() {
    add({ rid: nextRid(), key: "", url: "", auth_token: "", preset: false });
  }
  function update(rid: string, field: "key" | "url" | "auth_token", value: string) {
    onChange(rows.map((r) => (r.rid === rid ? { ...r, [field]: value } : r)));
  }
  function remove(rid: string) {
    onChange(rows.filter((r) => r.rid !== rid));
  }
  function toggle(rid: string) {
    setExpanded((s) => {
      const next = new Set(s);
      if (next.has(rid)) next.delete(rid);
      else next.add(rid);
      return next;
    });
  }

  return (
    <section className="border border-card-border/50 bg-card rounded-xl p-3 space-y-3">
      <div className="flex items-center justify-between gap-2">
        <SectionLabel description="HTTP MCP servers written to this provider's config on save.">
          mcp servers
        </SectionLabel>
        <DropdownMenu
          panelClassName="w-44"
          trigger={({ toggle: t }) => (
            <button
              type="button"
              onClick={t}
              className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-surface border border-border text-xs text-fg-muted transition-colors hover:text-fg hover:bg-surface-2 hover:border-border-focus/20"
            >
              <Plus size={13} /> Add MCP
            </button>
          )}
        >
          {({ close }) => (
            <div className="flex flex-col">
              {MCP_PRESETS.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  onClick={() => {
                    addPreset(p);
                    close();
                  }}
                  className="flex items-center gap-2 px-2 py-1.5 rounded text-xs text-fg hover:bg-surface-2"
                >
                  <span
                    style={{ backgroundColor: p.color }}
                    className="inline-flex items-center justify-center w-5 h-5 rounded text-white shrink-0"
                  >
                    <p.Icon className="w-3 h-3" />
                  </span>
                  {p.label}
                </button>
              ))}
              <button
                type="button"
                onClick={() => {
                  addOther();
                  close();
                }}
                className="flex items-center gap-2 px-2 py-1.5 rounded text-xs text-fg hover:bg-surface-2"
              >
                <span className="inline-flex items-center justify-center w-5 h-5 rounded bg-surface-2 text-fg-muted shrink-0">
                  <GenericProviderIcon className="w-3 h-3" />
                </span>
                Other
              </button>
            </div>
          )}
        </DropdownMenu>
      </div>

      {rows.length === 0 ? (
        <div className="text-fg-subtle text-xs italic">
          No MCP servers. Use "Add MCP" to add one.
        </div>
      ) : (
        <div className="space-y-2">
          {rows.map((row) => (
            <McpRow
              key={row.rid}
              row={row}
              open={expanded.has(row.rid)}
              onToggle={() => toggle(row.rid)}
              onUpdate={(field, value) => update(row.rid, field, value)}
              onRemove={() => remove(row.rid)}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function McpRow({
  row,
  open,
  onToggle,
  onUpdate,
  onRemove,
}: {
  row: McpRowState;
  open: boolean;
  onToggle: () => void;
  onUpdate: (field: "key" | "url" | "auth_token", value: string) => void;
  onRemove: () => void;
}) {
  const integration = matchIntegrations([row.url])[0];
  return (
    <div className="border border-border rounded-lg">
      <div className="flex items-center gap-2 px-2 py-1.5">
        <button
          type="button"
          onClick={onToggle}
          aria-expanded={open}
          className="flex items-center gap-2 min-w-0 flex-1 text-left"
        >
          {open ? (
            <ChevronDown size={14} className="shrink-0 text-fg-subtle" />
          ) : (
            <ChevronRight size={14} className="shrink-0 text-fg-subtle" />
          )}
          {integration ? (
            <IntegrationIcon integration={integration} size="sm" />
          ) : (
            <span className="inline-flex items-center justify-center w-6 h-6 rounded-md bg-surface-2 text-fg-muted shrink-0">
              <GenericProviderIcon className="w-3.5 h-3.5" />
            </span>
          )}
          <span className="text-xs text-fg truncate">{row.key || "new MCP"}</span>
          <span className="text-[11px] font-mono text-fg-subtle truncate hidden sm:inline">
            {row.url}
          </span>
        </button>
        <IconButton onClick={onRemove} title="Remove MCP">
          <Trash2 size={14} />
        </IconButton>
      </div>
      {open && (
        <div className="px-3 pb-3 pt-1 space-y-2 border-t border-border">
          <McpField label="Key">
            <TextInput
              value={row.key}
              onChange={(e) => onUpdate("key", e.target.value)}
              disabled={row.preset}
              placeholder="atlassian"
              mono
              padded
              className="w-full disabled:opacity-60"
            />
          </McpField>
          <McpField label="URL">
            <TextInput
              value={row.url}
              onChange={(e) => onUpdate("url", e.target.value)}
              disabled={row.preset}
              placeholder="https://…"
              mono
              padded
              className="w-full disabled:opacity-60"
            />
          </McpField>
          <McpField label="Auth Token">
            <TextInput
              value={row.auth_token}
              onChange={(e) => onUpdate("auth_token", e.target.value)}
              placeholder="(optional)"
              mono
              padded
              className="w-full"
            />
          </McpField>
        </div>
      )}
    </div>
  );
}

function McpField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block space-y-1">
      <span className="text-[10px] uppercase tracking-wide text-fg-subtle">{label}</span>
      {children}
    </label>
  );
}
