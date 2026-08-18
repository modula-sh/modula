import { useState } from "react";
import { AddProjects } from "../components/onboarding/AddProjects";
import { AddProviders } from "../components/onboarding/AddProviders";
// import { CreateUser } from "../components/onboarding/CreateUser";
import { Landing } from "../components/onboarding/Landing";
import {
  type LicenseAcceptance,
  LicenseAgreement,
} from "../components/onboarding/LicenseAgreement";
import { RequiredTools } from "../components/onboarding/RequiredTools";
import { WorkspaceStep } from "../components/onboarding/WorkspaceStep";
import { WindowControls } from "../components/WindowControls";
import type { useWorkspaceState } from "../hooks/useWorkspaceState";
import { useLocalStorage } from "../lib/useLocalStorage";

// "user" step temporarily disabled
const STEPS = ["landing", "license", "tools", "workspace", "providers", "projects"] as const;
type Step = (typeof STEPS)[number];

export function Onboarding({
  onComplete,
  wsState,
}: {
  onComplete: () => void;
  wsState: ReturnType<typeof useWorkspaceState>;
}) {
  const [step, setStep] = useState<Step>("landing");
  const [ws, setWs] = useState<string | null>(null);
  const [, setLicenseAcceptance] = useLocalStorage<LicenseAcceptance | null>(
    "modula.licenseAccepted",
    null,
  );
  const idx = STEPS.indexOf(step);
  const onBack = idx > 0 ? () => setStep(STEPS[idx - 1]) : undefined;

  let content;
  if (step === "landing") {
    content = <Landing onNext={() => setStep("license")} onBack={onBack} />;
  } else if (step === "license") {
    content = (
      <LicenseAgreement
        onBack={onBack}
        onAccept={(acceptance) => {
          setLicenseAcceptance(acceptance);
          setStep("tools");
        }}
      />
    );
    // } else if (step === "user") {
    //   content = <CreateUser onNext={() => setStep("tools")} onBack={onBack} />;
  } else if (step === "tools") {
    content = <RequiredTools onNext={() => setStep("workspace")} onBack={onBack} />;
  } else if (step === "workspace") {
    content = (
      <WorkspaceStep
        workspaces={wsState.workspaces}
        activeId={wsState.workspace}
        loaded={wsState.loaded}
        onBack={onBack}
        onContinue={(id) => {
          wsState.setWorkspace(id);
          setWs(id);
          setStep("providers");
        }}
        onCreated={(created) => {
          wsState.addWorkspace(created);
          wsState.setWorkspace(created.id);
          setWs(created.id);
          setStep("providers");
        }}
      />
    );
  } else if (step === "providers") {
    content = <AddProviders ws={ws!} onNext={() => setStep("projects")} onBack={onBack} />;
  } else {
    content = <AddProjects ws={ws!} onNext={onComplete} onBack={onBack} />;
  }

  return (
    <main className="h-screen w-screen flex flex-col bg-bg text-fg">
      {/* Onboarding runs before the Titlebar mounts, so it carries the buttons. */}
      <div className="shrink-0 flex justify-end">
        <WindowControls />
      </div>
      <div className="flex-1 flex flex-col items-center justify-center gap-6">{content}</div>
    </main>
  );
}
