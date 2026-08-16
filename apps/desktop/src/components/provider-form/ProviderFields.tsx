import { PROVIDER_TYPES } from "../../lib/providerTypes";
import { DropdownSelect } from "../DropdownMenu";
import { FieldRow } from "../FieldRow";
import { FileInput } from "../FileInput";
import { TextInput } from "../TextInput";
import { defaultConfigDir } from "./useProviderForm";

/** One editable MCP row. `preset` marks a row added from a first-class preset
 * (Key + URL locked); `rid` is a stable client-only React key. Neither field
 * is sent to the server. */
export interface McpRowState {
  rid: string;
  key: string;
  url: string;
  auth_token: string;
  preset: boolean;
}

export interface ProviderFormState {
  name: string;
  type: string;
  config_dir: string;
  description: string;
  mcp_servers: McpRowState[];
}

/** Name / type / config-dir / description inputs for a provider, shared by the
 * in-app editor and onboarding. Selecting a type resets config dir to that
 * type's default. */
export function ProviderFields({
  state,
  onChange,
  autoFocus = false,
  className = "",
}: {
  state: ProviderFormState;
  onChange: <K extends keyof ProviderFormState>(key: K, value: ProviderFormState[K]) => void;
  autoFocus?: boolean;
  className?: string;
}) {
  return (
    <section className={`border border-card-border/50 bg-card rounded-xl px-3 ${className}`.trim()}>
      <FieldRow label="name" description="Display name shown throughout the dashboard.">
        <TextInput
          value={state.name}
          onChange={(e) => onChange("name", e.target.value)}
          placeholder="Display name"
          padded
          className="w-full"
          autoFocus={autoFocus}
        />
      </FieldRow>
      <FieldRow
        label="type"
        description="Which CLI tool this provider drives (claude, codex, opencode)."
      >
        <DropdownSelect
          variant="field"
          padded
          className="w-full"
          value={state.type}
          onChange={(v) => {
            onChange("type", v);
            onChange("config_dir", defaultConfigDir(v));
          }}
          options={PROVIDER_TYPES.map((t) => ({ value: t.id, label: t.label }))}
        />
      </FieldRow>
      <FieldRow
        label="config dir"
        description="Directory the provider reads its config and credentials from."
      >
        <FileInput
          value={state.config_dir}
          onChange={(v) => onChange("config_dir", v)}
          directory
          placeholder={defaultConfigDir(state.type)}
          mono
          padded
          className="w-full"
        />
      </FieldRow>
      <FieldRow
        label="description"
        description="Optional short summary, surfaced on the providers list."
      >
        <TextInput
          value={state.description}
          onChange={(e) => onChange("description", e.target.value)}
          placeholder="(optional)"
          padded
          className="w-full"
        />
      </FieldRow>
    </section>
  );
}
