import { AtlassianIcon } from "../components/icons/AtlassianIcon";
import { CloudflareIcon } from "../components/icons/CloudflareIcon";
import { FigmaIcon } from "../components/icons/FigmaIcon";
import { GitHubIcon } from "../components/icons/GitHubIcon";
import { LinearIcon } from "../components/icons/LinearIcon";
import { NotionIcon } from "../components/icons/NotionIcon";
import { SentryIcon } from "../components/icons/SentryIcon";
import { SlackIcon } from "../components/icons/SlackIcon";
import { StripeIcon } from "../components/icons/StripeIcon";
import { SupabaseIcon } from "../components/icons/SupabaseIcon";
import { VercelIcon } from "../components/icons/VercelIcon";
import registry from "./mcpIntegrations.json";

type IconComponent = (props: { className?: string }) => React.ReactElement;

/** Maps the `id` in mcpIntegrations.json to its icon component. New
 *  integration = add a file under components/icons + an entry here + a row
 *  in the JSON. */
const ICONS: Record<string, IconComponent> = {
  atlassian: AtlassianIcon,
  linear: LinearIcon,
  github: GitHubIcon,
  notion: NotionIcon,
  slack: SlackIcon,
  sentry: SentryIcon,
  stripe: StripeIcon,
  cloudflare: CloudflareIcon,
  supabase: SupabaseIcon,
  vercel: VercelIcon,
  figma: FigmaIcon,
};

export interface McpIntegration {
  id: string;
  label: string;
  color: string;
  pattern: RegExp;
  Icon: IconComponent;
}

interface RegistryEntry {
  id: string;
  label: string;
  color: string;
  pattern: string;
}

export const MCP_INTEGRATIONS: McpIntegration[] = (registry as RegistryEntry[])
  .filter((e) => ICONS[e.id])
  .map((e) => ({
    id: e.id,
    label: e.label,
    color: e.color,
    pattern: new RegExp(e.pattern, "i"),
    Icon: ICONS[e.id],
  }));

export function matchIntegrations(endpoints: string[]): McpIntegration[] {
  return MCP_INTEGRATIONS.filter((integ) => endpoints.some((e) => integ.pattern.test(e)));
}

export interface McpPreset {
  id: string;
  label: string;
  key: string;
  url: string;
  color: string;
  Icon: IconComponent;
}

/** First-class one-click MCP servers offered by the provider form. Keys/URLs
 * are fixed by the product spec; icons and colors reuse the integration
 * registry. */
export const MCP_PRESETS: McpPreset[] = [
  { id: "atlassian", label: "Jira", key: "atlassian", url: "https://mcp.atlassian.com/v1/mcp" },
  { id: "linear", label: "Linear", key: "linear", url: "https://mcp.linear.app/mcp" },
  { id: "github", label: "GitHub", key: "github", url: "https://api.githubcopilot.com/mcp/" },
].map((p) => {
  const reg = (registry as RegistryEntry[]).find((e) => e.id === p.id);
  return { ...p, color: reg?.color ?? "#64748b", Icon: ICONS[p.id] };
});

export function IntegrationIcon({
  integration,
  size = "md",
}: {
  integration: McpIntegration;
  size?: "sm" | "md";
}) {
  const dims = size === "sm" ? "w-6 h-6" : "w-7 h-7";
  const inner = size === "sm" ? "w-3.5 h-3.5" : "w-4 h-4";
  const { Icon, label, color } = integration;
  return (
    <span
      title={label}
      style={{ backgroundColor: color }}
      className={`inline-flex items-center justify-center ${dims} rounded-md text-white shadow-sm ring-1 ring-black/5`}
    >
      <Icon className={inner} />
    </span>
  );
}

export function IntegrationIconRow({
  integrations,
  size = "md",
}: {
  integrations: McpIntegration[];
  size?: "sm" | "md";
}) {
  if (integrations.length === 0) return null;
  return (
    <div className="flex items-center gap-1.5 shrink-0">
      {integrations.map((i) => (
        <IntegrationIcon key={i.id} integration={i} size={size} />
      ))}
    </div>
  );
}
