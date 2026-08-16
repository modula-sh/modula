import { useContext } from "react";
import { useNavigate } from "react-router-dom";
import { Button } from "../components/Button";
import { HeaderSlot } from "../components/HeaderSlot";
import { ProjectCard } from "../components/ProjectCard";
import { WorkspaceContext } from "../contexts/WorkspaceContext";
import { useProjects } from "../queries/project";

export function ProjectsView() {
  const ws = useContext(WorkspaceContext);
  const navigate = useNavigate();
  const { data: projects, isPending } = useProjects(ws);

  return (
    <main className="flex-1 overflow-y-auto p-4 space-y-4">
      <HeaderSlot>
        <Button className="ml-auto" onClick={() => navigate("/projects/new")}>
          + New Project
        </Button>
      </HeaderSlot>
      {isPending ? (
        <div className="text-fg-subtle text-sm">loading projects…</div>
      ) : !projects || projects.length === 0 ? (
        <div className="flex flex-col items-center text-center gap-1 py-24 font-inter">
          <div className="text-fg-muted text-sm">No projects</div>
          <div className="text-fg-subtle text-xs">
            Add a project to start tracking variants and worktrees.
          </div>
        </div>
      ) : (
        <div className="space-y-3">
          {projects.map((p) => (
            <ProjectCard
              key={p.name}
              project={p}
              onOpen={() => navigate(`/projects/edit/${p.id}`)}
            />
          ))}
        </div>
      )}
    </main>
  );
}
