import { useQueryClient } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { useState } from "react";
import { useFeedback } from "../../hooks/useFeedback";
import { projectKeys, useProjects } from "../../queries/project";
import { errorMessage } from "../../services/client";
import type { Project } from "../../types";
import { FeedbackText } from "../FeedbackText";
import { LargeButton } from "../LargeButton";
import { ProjectCard } from "../ProjectCard";
import { ProjectFields } from "../project-form/ProjectFields";
import { createProject, updateProject, useProjectForm } from "../project-form/useProjectForm";
import { OnboardingActions } from "./OnboardingActions";
import { OnboardingTitle } from "./OnboardingTitle";

export function AddProjects({
  ws,
  onNext,
  onBack,
}: {
  ws: string;
  onNext: () => void;
  onBack?: () => void;
}) {
  const queryClient = useQueryClient();
  const { data: projects } = useProjects(ws);
  const [form, setForm] = useState<{ project: Project | null } | null>(null);

  if (form) {
    return (
      <ProjectFormView
        ws={ws}
        project={form.project}
        onBack={() => setForm(null)}
        onSaved={() => {
          setForm(null);
          queryClient.invalidateQueries({ queryKey: projectKeys.all(ws) });
        }}
      />
    );
  }

  const hasProjects = !!projects && projects.length > 0;

  return (
    <>
      <OnboardingTitle>Add Projects</OnboardingTitle>
      <section className="w-[32rem] flex flex-col gap-2 font-inter">
        {projects && !hasProjects && (
          <p className="text-fg-muted text-sm text-center py-2">No Projects</p>
        )}
        {projects?.map((p) => (
          <ProjectCard key={p.id} project={p} onOpen={() => setForm({ project: p })} />
        ))}
        <button
          type="button"
          onClick={() => setForm({ project: null })}
          className="flex items-center justify-center gap-2 border border-border rounded-xl p-3 text-sm text-fg-muted hover:text-fg hover:border-border-focus transition-colors"
        >
          <Plus size={16} />
          Add Project
        </button>
      </section>
      <OnboardingActions onBack={onBack} className="mt-4">
        <LargeButton onClick={onNext}>{hasProjects ? "Next" : "Skip for now"}</LargeButton>
      </OnboardingActions>
    </>
  );
}

function ProjectFormView({
  ws,
  project,
  onBack,
  onSaved,
}: {
  ws: string;
  project: Project | null;
  onBack: () => void;
  onSaved: () => void;
}) {
  const { state, patch, isCreate, valid, branches } = useProjectForm(ws, project);
  const [busy, setBusy] = useState(false);
  const fb = useFeedback();

  async function submit() {
    if (!valid || busy) return;
    setBusy(true);
    fb.clear();
    try {
      if (isCreate) {
        await createProject(ws, state);
      } else {
        await updateProject(ws, project!.id, state);
      }
      onSaved();
    } catch (e: unknown) {
      fb.err(errorMessage(e));
      setBusy(false);
    }
  }

  return (
    <>
      <OnboardingTitle>{isCreate ? "New project" : project!.name}</OnboardingTitle>
      <ProjectFields
        state={state}
        onChange={patch}
        isCreate={isCreate}
        branches={branches}
        autoFocus
        className="w-[32rem] font-inter"
      />
      <OnboardingActions onBack={onBack} className="mt-4">
        <LargeButton onClick={submit} disabled={!valid || busy}>
          {busy ? "Saving…" : isCreate ? "Add" : "Save"}
        </LargeButton>
      </OnboardingActions>
      <FeedbackText feedback={fb.feedback} />
    </>
  );
}
