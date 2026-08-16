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

export function ProviderTypeIcon({
  type,
  size = "md",
}: {
  type: string | null | undefined;
  size?: "xs" | "sm" | "md";
}) {
  const t = providerType(type);
  const dims = size === "xs" ? "w-5 h-5" : size === "sm" ? "w-6 h-6" : "w-7 h-7";
  const inner = size === "xs" ? "w-3 h-3" : size === "sm" ? "w-3.5 h-3.5" : "w-4 h-4";
  return (
    <span
      title={t.label}
      style={{ backgroundColor: t.color }}
      className={`inline-flex items-center justify-center ${dims} rounded-md text-white shadow-sm ring-1 ring-black/5 shrink-0`}
    >
      <t.Icon className={inner} />
    </span>
  );
}
