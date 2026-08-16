import { IntegrationIconRow, matchIntegrations } from "../lib/mcpIntegrations";
import { ProviderTypeIcon } from "../lib/providerTypes";
import type { ProviderSummary } from "../types";
import { AgentIdenticon } from "./AgentIdenticon";
import { Pill } from "./Pill";

export function ProviderCard({
  provider,
  onOpen,
}: {
  provider: ProviderSummary;
  onOpen: () => void;
}) {
  const integrations = matchIntegrations(provider.mcp_endpoints);
  const agents = provider.agents_using;

  return (
    <article
      onClick={onOpen}
      className="border border-card-border/50 rounded-xl p-3 cursor-pointer bg-card hover:bg-surface/40 transition-colors"
    >
      <div className="flex items-center gap-3">
        <ProviderTypeIcon type={provider.type} />
        <div className="flex-1 min-w-0 flex items-center gap-2 flex-wrap">
          <span className="font-inter font-medium text-fg truncate">{provider.name}</span>
          {!provider.config_dir_exists && <Pill tone="red">config dir missing</Pill>}
        </div>
        <IntegrationIconRow integrations={integrations} />
      </div>

      {provider.description && (
        <p className="font-inter text-xs text-fg-muted mt-3 line-clamp-2">{provider.description}</p>
      )}

      {(provider.mcp_server_count > 0 || agents.length > 0) && (
        <div className="mt-3 space-y-1.5 text-[11px]">
          {provider.mcp_server_count > 0 && (
            <div className="flex flex-wrap items-center gap-1.5">
              <Pill size="sm">MCP {provider.mcp_server_count}</Pill>
            </div>
          )}
          {agents.length > 0 && (
            <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
              {agents.map((name) => (
                <span key={name} className="inline-flex items-center gap-1.5">
                  <span className="inline-flex items-center justify-center w-5 h-5 rounded-sm bg-surface-2 text-fg border border-border shrink-0">
                    <AgentIdenticon id={name} size={14} />
                  </span>
                  <span className="text-fg">{name}</span>
                </span>
              ))}
            </div>
          )}
        </div>
      )}
    </article>
  );
}
