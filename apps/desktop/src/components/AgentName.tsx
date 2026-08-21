import { useSnapshot } from "../contexts/SnapshotContext";
import { ProviderTypeIcon } from "../lib/providerTypes";
import { AgentIdenticon } from "./AgentIdenticon";

/** Identicon frame + glyph, sized to sit level with the provider icon beside it. */
const FRAME = { "2xs": "w-4 h-4", xs: "w-5 h-5" };
const GLYPH = { "2xs": 11, xs: 14 };

/** An agent's identicon, provider icon, and name — the run-list echo of the
 *  agent tile's header. Rows only carry `agent_name`, so the provider is looked
 *  up in the snapshot; a deleted agent keeps its identicon (derived from the
 *  name alone) but drops the icon rather than guessing at a provider. */
export function AgentName({
  name,
  iconSize = "xs",
  className = "",
}: {
  name: string;
  iconSize?: keyof typeof FRAME;
  className?: string;
}) {
  const { snap } = useSnapshot();
  const agent = snap?.config.agents.find((a) => a.name === name);
  const provider = agent?.provider_id
    ? (snap?.config.providers.find((p) => p.id === agent.provider_id) ?? null)
    : null;

  return (
    <span className={`inline-flex items-center gap-1.5 min-w-0 font-inter text-fg ${className}`}>
      <span
        className={`inline-flex items-center justify-center ${FRAME[iconSize]} rounded-[3px] bg-surface-2 text-fg border border-border shrink-0`}
        aria-hidden
      >
        <AgentIdenticon id={name} size={GLYPH[iconSize]} />
      </span>
      {agent?.provider_id && (
        <ProviderTypeIcon
          type={provider?.type}
          size={iconSize}
          title={provider?.name ?? agent.provider_id}
        />
      )}
      <span className="min-w-0 truncate">{name}</span>
    </span>
  );
}
