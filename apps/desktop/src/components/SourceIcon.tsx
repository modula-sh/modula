import { GitHubIcon } from "./icons/GitHubIcon";
import { JiraIcon } from "./icons/JiraIcon";
import { LinearIcon } from "./icons/LinearIcon";
import { ModulaIcon } from "./icons/ModulaIcon";

type IconComponent = (props: { className?: string }) => React.ReactElement;

// Source-agnostic registry: a task's `source` string ('jira', 'linear', …) maps
// to its tracker icon + brand tint + brand-cased display name. Internal tasks
// ('internal') get the Modula brandmark; other unknown sources render no icon
// and display their raw value.
const SOURCE_ICONS: Record<string, { Icon: IconComponent; tint: string; label: string }> = {
  jira: { Icon: JiraIcon, tint: "text-[#2684FF]", label: "Jira" },
  linear: { Icon: LinearIcon, tint: "text-fg-muted", label: "Linear" },
  github: { Icon: GitHubIcon, tint: "text-fg", label: "GitHub" },
  internal: { Icon: ModulaIcon, tint: "text-fg", label: "Modula" },
};

/** Brand-cased name for a task source ('jira' → 'Jira'); falls back to the raw
 *  value for unknown sources. */
export function sourceDisplayName(source: string | null | undefined): string {
  const s = (source ?? "").trim();
  return SOURCE_ICONS[s.toLowerCase()]?.label ?? s;
}

/** Small tracker icon for an external task source. Returns null when the source
 *  is internal/unknown, so callers can drop it inline unconditionally. */
export function SourceIcon({
  source,
  className = "",
}: {
  source: string | null | undefined;
  className?: string;
}) {
  const entry = SOURCE_ICONS[(source ?? "").trim().toLowerCase()];
  if (!entry) return null;
  return <entry.Icon className={`${entry.tint} ${className}`.trim()} />;
}
