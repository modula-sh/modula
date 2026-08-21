import { ClaudeIcon } from "../components/icons/ClaudeIcon";
import { CodexIcon } from "../components/icons/CodexIcon";
import { GenericProviderIcon } from "../components/icons/GenericProviderIcon";
import { OpenCodeIcon } from "../components/icons/OpenCodeIcon";

type IconComponent = (props: { className?: string }) => React.ReactElement;

export interface ProviderType {
  id: string;
  label: string;
  color: string;
  Icon: IconComponent;
}

export const PROVIDER_TYPES: ProviderType[] = [
  { id: "claude", label: "Claude", color: "#D97757", Icon: ClaudeIcon },
  { id: "codex", label: "Codex", color: "#000000", Icon: CodexIcon },
  { id: "opencode", label: "OpenCode", color: "#4B4646", Icon: OpenCodeIcon },
];

const BY_ID: Record<string, ProviderType> = Object.fromEntries(
  PROVIDER_TYPES.map((t) => [t.id, t]),
);

const FALLBACK: ProviderType = {
  id: "generic",
  label: "Provider",
  color: "#6B7280",
  Icon: GenericProviderIcon,
};

/** Missing / empty type → `claude` (the DB default). Truly unknown ids
 *  (e.g. a future type the frontend hasn't shipped icons for) fall through
 *  to the generic tile. */
export function providerType(id: string | null | undefined): ProviderType {
  const key = id && id.trim() ? id : "claude";
  return BY_ID[key] ?? FALLBACK;
}

const DIMS = { "2xs": "w-4 h-4", xs: "w-5 h-5", sm: "w-6 h-6", md: "w-7 h-7" };
const INNER = { "2xs": "w-2.5 h-2.5", xs: "w-3 h-3", sm: "w-3.5 h-3.5", md: "w-4 h-4" };

export function ProviderTypeIcon({
  type,
  size = "md",
  title,
}: {
  type: string | null | undefined;
  /** `2xs` for dense row contexts (run lists). */
  size?: keyof typeof DIMS;
  /** Overrides the type label as the tooltip — e.g. the provider's own name. */
  title?: string;
}) {
  const t = providerType(type);
  return (
    <span
      title={title ?? t.label}
      style={{ backgroundColor: t.color }}
      className={`inline-flex items-center justify-center ${DIMS[size]} rounded-md text-white shadow-sm ring-1 ring-black/5 shrink-0`}
    >
      <t.Icon className={INNER[size]} />
    </span>
  );
}
