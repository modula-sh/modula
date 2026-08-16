import { ArrowUpRight, Check } from "lucide-react";
import { useSystemTools } from "../../queries/system";
import { errorMessage } from "../../services/client";
import { ClaudeIcon } from "../icons/ClaudeIcon";
import { CodexIcon } from "../icons/CodexIcon";
import { GitHubCliIcon } from "../icons/GitHubCliIcon";
import { OpenCodeIcon } from "../icons/OpenCodeIcon";
import { openUrl } from "../openUrl";

const TOOLS: Record<
  string,
  { name: string; Icon: (props: { className?: string }) => React.ReactElement; installUrl: string }
> = {
  gh: { name: "GitHub CLI", Icon: GitHubCliIcon, installUrl: "https://cli.github.com" },
  claude: { name: "Claude", Icon: ClaudeIcon, installUrl: "https://code.claude.com/docs/en/setup" },
  codex: { name: "Codex", Icon: CodexIcon, installUrl: "https://developers.openai.com/codex/cli" },
  opencode: { name: "OpenCode", Icon: OpenCodeIcon, installUrl: "https://opencode.ai" },
};

/**
 * Renders the list of recommended tools with live install status.
 * Shared between the onboarding flow and the settings page.
 */
export function RecommendedToolsList({ className }: { className?: string }) {
  const { data: tools, error } = useSystemTools();

  return (
    <div className={`flex flex-col gap-2 ${className ?? ""}`}>
      {error && <p className="text-sm text-red-400">{errorMessage(error)}</p>}
      {tools?.map(({ id, installed }) => {
        const tool = TOOLS[id];
        if (!tool) return null;
        const { name, Icon, installUrl } = tool;
        if (installed) {
          return (
            <article key={id} className="border border-card-border/50 bg-card rounded-xl p-3">
              <div className="flex items-center gap-3">
                <span className="inline-flex items-center justify-center w-7 h-7 rounded-md bg-surface-2 text-fg border border-border shrink-0">
                  <Icon className="w-4 h-4" />
                </span>
                <span className="font-semibold text-fg">{name}</span>
                <span className="ml-auto shrink-0 inline-flex items-center gap-1 text-green-400 text-sm">
                  <Check size={14} />
                  INSTALLED
                </span>
              </div>
            </article>
          );
        }
        return (
          <button
            key={id}
            type="button"
            onClick={() => openUrl(installUrl)}
            className="group block w-full text-left border border-card-border/50 bg-card rounded-xl p-3 transition-colors hover:bg-surface/40"
          >
            <div className="flex items-center gap-3">
              <span className="inline-flex items-center justify-center w-7 h-7 rounded-md bg-surface-2 text-fg border border-border shrink-0">
                <Icon className="w-4 h-4" />
              </span>
              <div className="flex flex-col">
                <span className="font-semibold text-fg">{name}</span>
                <span className="text-fg-muted text-xs">Click to open the install page</span>
              </div>
              <span className="ml-auto shrink-0 inline-flex items-center gap-1 text-fg-muted group-hover:text-fg text-sm transition-colors">
                NOT INSTALLED
                <ArrowUpRight size={14} />
              </span>
            </div>
          </button>
        );
      })}
    </div>
  );
}
