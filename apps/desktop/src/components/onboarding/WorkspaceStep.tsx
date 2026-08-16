import { Boxes } from "lucide-react";
import type { WorkspaceInfo } from "../../types";
import { LargeButton } from "../LargeButton";
import { CreateWorkspace } from "./CreateWorkspace";
import { OnboardingActions } from "./OnboardingActions";
import { OnboardingTitle } from "./OnboardingTitle";

/** Workspace onboarding step. When a workspace already exists (e.g. corrupted
 * app state that reset the onboarding flag), show it instead of the create
 * form so the user can continue rather than being forced to make a duplicate. */
export function WorkspaceStep({
  workspaces,
  activeId,
  loaded,
  onContinue,
  onCreated,
  onBack,
}: {
  workspaces: WorkspaceInfo[];
  activeId: string;
  loaded: boolean;
  onContinue: (wsId: string) => void;
  onCreated: (ws: WorkspaceInfo) => void;
  onBack?: () => void;
}) {
  if (!loaded) {
    return <span className="text-fg-subtle text-sm font-inter">loading…</span>;
  }
  if (workspaces.length === 0) {
    return <CreateWorkspace onBack={onBack} onCreated={onCreated} />;
  }

  const current = workspaces.find((w) => w.id === activeId) ?? workspaces[0];

  return (
    <>
      <OnboardingTitle>Workspace</OnboardingTitle>
      <section className="w-[32rem] flex flex-col gap-2 font-inter">
        {workspaces.map((w) => (
          <article key={w.id} className="border border-border rounded-xl p-3">
            <div className="flex items-center gap-3">
              <span className="inline-flex items-center justify-center w-7 h-7 rounded-md bg-surface-2 text-fg-muted border border-border shrink-0">
                <Boxes size={16} />
              </span>
              <div className="min-w-0">
                <div className="font-semibold text-fg truncate">{w.name}</div>
                {w.description && (
                  <div className="text-fg-muted text-xs truncate">{w.description}</div>
                )}
              </div>
            </div>
          </article>
        ))}
      </section>
      <OnboardingActions onBack={onBack} className="mt-4">
        <LargeButton onClick={() => onContinue(current.id)}>Continue</LargeButton>
      </OnboardingActions>
    </>
  );
}
