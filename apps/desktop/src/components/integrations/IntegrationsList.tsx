import { useContext, useState } from "react";
import { WorkspaceContext } from "../../contexts/WorkspaceContext";
import { useIntegrations } from "../../queries/integration";
import { errorMessage } from "../../services/client";
import { GitHubIcon } from "../icons/GitHubIcon";
import { JiraIcon } from "../icons/JiraIcon";
import { LinearIcon } from "../icons/LinearIcon";
import { Pill } from "../Pill";
import { IntegrationModal } from "./IntegrationModal";

export const INTEGRATIONS: {
  id: string;
  name: string;
  Icon: (props: { className?: string }) => React.ReactElement;
}[] = [
  { id: "github", name: "GitHub", Icon: GitHubIcon },
  { id: "jira", name: "Jira", Icon: JiraIcon },
  { id: "linear", name: "Linear", Icon: LinearIcon },
];

export function IntegrationsList() {
  const ws = useContext(WorkspaceContext);
  const { data: integrations, error } = useIntegrations(ws);
  const [openId, setOpenId] = useState<string | null>(null);

  return (
    <div className="flex flex-col gap-2">
      {error && <p className="text-sm text-red-400">{errorMessage(error)}</p>}
      {INTEGRATIONS.map(({ id, name, Icon }) => {
        const connected = integrations?.find((i) => i.id === id);
        return (
          <button
            key={id}
            type="button"
            onClick={() => setOpenId(id)}
            className="block w-full text-left border border-card-border/50 bg-card rounded-xl p-3 transition-colors hover:bg-surface/40"
          >
            <div className="flex items-center gap-3">
              <span className="inline-flex items-center justify-center w-7 h-7 rounded-md bg-surface-2 text-fg border border-border shrink-0">
                <Icon className="w-4 h-4" />
              </span>
              <span className="font-semibold text-fg">{name}</span>
              <span className="ml-auto shrink-0">
                {connected ? (
                  <Pill tone="green">connected</Pill>
                ) : (
                  <span className="text-fg-muted text-xs uppercase tracking-wide">
                    not connected
                  </span>
                )}
              </span>
            </div>
          </button>
        );
      })}
      {openId && (
        <IntegrationModal
          workspace={ws}
          id={openId}
          existing={integrations?.find((i) => i.id === openId)}
          onClose={() => setOpenId(null)}
        />
      )}
    </div>
  );
}
