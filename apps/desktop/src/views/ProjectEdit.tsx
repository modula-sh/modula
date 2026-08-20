import { useContext, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { Button } from "../components/Button";
import { EditPageFooter } from "../components/EditPageFooter";
import { FeedbackText } from "../components/FeedbackText";
import { Pill } from "../components/Pill";
import { ProjectFields } from "../components/project-form/ProjectFields";
import {
  createProject,
  updateProject,
  useProjectForm,
} from "../components/project-form/useProjectForm";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useFeedback } from "../hooks/useFeedback";
import { useProject } from "../queries/project";
import { client, errorMessage } from "../services/client";
import type { Project } from "../types";

export function ProjectEditPage() {
  const ws = useContext(WorkspaceContext);
  const { id } = useParams<{ id: string }>();
  const { data: detail, error, isLoading } = useProject(ws, id);

  if (error) {
    return (
      <main className="flex-1 flex items-center justify-center text-fg-muted">
        <div className="text-red-400">{errorMessage(error)}</div>
      </main>
    );
  }
  if (isLoading) {
    return <main className="flex-1 flex items-center justify-center text-fg-subtle">loading…</main>;
  }
  return <ProjectForm detail={detail ?? null} />;
}

function ProjectForm({ detail }: { detail: Project | null }) {
  const ws = useContext(WorkspaceContext);
  const navigate = useNavigate();
  const { state, patch, isCreate, valid, branches } = useProjectForm(ws, detail);
  const [busy, setBusy] = useState(false);
  const fb = useFeedback();

  async function save() {
    setBusy(true);
    fb.clear();
    try {
      if (isCreate) {
        const out = await createProject(ws, state);
        navigate(`/projects/edit/${out.id}`);
      } else {
        await updateProject(ws, detail!.id, state);
        fb.ok("saved", { clearAfter: 4000 });
      }
    } catch (e: unknown) {
      fb.err(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!detail) return;
    if (
      !confirm(
        `Delete project ${detail.name}? Active worktrees are not removed; references in spec markdown are not tracked.`,
      )
    ) {
      return;
    }
    setBusy(true);
    fb.clear();
    try {
      await client.project.delete(ws, detail.id);
      navigate("/projects");
    } catch (e: unknown) {
      fb.err(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const canSave = !busy && valid;

  return (
    <main className="flex-1 overflow-y-auto px-4 py-8 font-inter">
      <div className="max-w-4xl mx-auto space-y-8">
        <header className="space-y-1">
          <div className="flex items-center gap-2 flex-wrap">
            <h1 className="text-lg font-semibold text-fg">
              {isCreate ? "New project" : detail!.name}
            </h1>
            {detail && !detail.exists && <Pill tone="red">missing on disk</Pill>}
          </div>
        </header>

        <section>
          <ProjectFields
            state={state}
            onChange={patch}
            isCreate={isCreate}
            branches={branches}
            autoFocus
          />
        </section>

        <EditPageFooter>
          <Button onClick={save} disabled={!canSave}>
            {busy ? "saving…" : isCreate ? "Create" : "Save"}
          </Button>
          {!isCreate && (
            <Button onClick={remove} disabled={busy}>
              Delete
            </Button>
          )}
          <Button tone="link" onClick={() => navigate("/projects")} disabled={busy}>
            Cancel
          </Button>
          <FeedbackText feedback={fb.feedback} />
        </EditPageFooter>
      </div>
    </main>
  );
}
