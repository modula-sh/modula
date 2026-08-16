import { useCallback, useEffect, useState } from "react";
import { client } from "../../services/client";
import type { ProviderMcpEntry, ProviderSummary } from "../../types";
import type { McpRowState, ProviderFormState } from "./ProviderFields";

export function defaultConfigDir(type: string): string {
  switch (type) {
    case "opencode":
      return "~/.config/opencode";
    case "codex":
      return "~/.codex";
    default:
      return "~/.claude";
  }
}

let ridCounter = 0;
export function nextRid(): string {
  ridCounter += 1;
  return `mcp-${ridCounter}`;
}

function seedRows(p: ProviderSummary): McpRowState[] {
  const servers = "mcp_servers" in p ? (p as { mcp_servers: ProviderMcpEntry[] }).mcp_servers : [];
  return servers.map((s) => ({
    rid: nextRid(),
    key: s.key,
    url: s.url,
    auth_token: s.auth_token ?? "",
    preset: false,
  }));
}

function emptyFormState(): ProviderFormState {
  return {
    name: "",
    type: "claude",
    config_dir: defaultConfigDir("claude"),
    description: "",
    mcp_servers: [],
  };
}

function formStateFrom(p: ProviderSummary): ProviderFormState {
  return {
    name: p.name,
    type: p.type || "claude",
    config_dir: p.config_dir ?? "",
    description: p.description ?? "",
    mcp_servers: seedRows(p),
  };
}

/** Form state for the shared provider fields. Pass an existing provider to
 * edit, or `null` to create. Accepts a `ProviderSummary`, which the in-app
 * `ProviderDetail` also satisfies. */
export function useProviderForm(detail: ProviderSummary | null) {
  const isCreate = detail === null;
  const [state, setState] = useState<ProviderFormState>(() =>
    detail ? formStateFrom(detail) : emptyFormState(),
  );

  useEffect(() => {
    setState(detail ? formStateFrom(detail) : emptyFormState());
  }, [detail]);

  const patch = useCallback(<K extends keyof ProviderFormState>(k: K, v: ProviderFormState[K]) => {
    setState((s) => ({ ...s, [k]: v }));
  }, []);

  const keys = state.mcp_servers.map((r) => r.key.trim());
  const mcpValid =
    state.mcp_servers.every((r) => r.key.trim() && r.url.trim()) &&
    new Set(keys).size === keys.length;
  const valid = !!state.name.trim() && !!state.config_dir.trim() && mcpValid;

  return { state, patch, isCreate, valid };
}

function payload(state: ProviderFormState, manageMcp: boolean) {
  const base = {
    name: state.name.trim(),
    type: state.type,
    config_dir: state.config_dir.trim(),
    description: state.description.trim() || null,
  };
  if (!manageMcp) return base;
  const mcp_servers: ProviderMcpEntry[] = state.mcp_servers.map((r) => ({
    key: r.key.trim(),
    url: r.url.trim(),
    auth_token: r.auth_token.trim() || null,
  }));
  return { ...base, mcp_servers };
}

export function createProvider(
  ws: string,
  state: ProviderFormState,
  manageMcp = false,
): Promise<{ id: string }> {
  return client.provider.create(ws, payload(state, manageMcp));
}

export async function updateProvider(
  ws: string,
  id: string,
  state: ProviderFormState,
  manageMcp = false,
): Promise<void> {
  await client.provider.update(ws, id, payload(state, manageMcp));
}
