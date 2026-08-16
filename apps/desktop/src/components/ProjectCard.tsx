import { ChevronRight, Folder, GitBranch } from "lucide-react";
import type { Project } from "../types";
import { Pill } from "./Pill";

export function ProjectCard({ project, onOpen }: { project: Project; onOpen: () => void }) {
  const wt = project.worktrees.length;
  return (
    <article
      onClick={onOpen}
      className="group border border-card-border/50 rounded-xl p-3 cursor-pointer bg-card hover:bg-surface/40 transition-colors"
    >
      <div className="flex items-center gap-3">
        <span
          className="inline-flex items-center justify-center w-7 h-7 rounded-md bg-surface-2 text-fg-muted border border-border shrink-0"
          aria-hidden
        >
          <Folder size={16} />
        </span>

        <div className="flex-1 min-w-0 flex items-center gap-2 flex-wrap">
          <span className="font-inter font-medium text-fg truncate">{project.name}</span>
          {project.base_branch && (
            <Pill variant="flat">
              <GitBranch size={11} className="shrink-0 text-fg-subtle" />
              {project.base_branch}
            </Pill>
          )}
          {!project.exists && <Pill tone="red">missing on disk</Pill>}
        </div>

        {wt > 0 && (
          <span
            className="text-[11px] text-fg-muted shrink-0 inline-flex items-center gap-1 font-mono"
            title={`${wt} worktree${wt === 1 ? "" : "s"}`}
          >
            <GitBranch size={14} />
            {wt}
          </span>
        )}

        <ChevronRight
          size={16}
          className="text-fg-subtle/40 shrink-0 transition-all group-hover:text-fg-muted group-hover:translate-x-0.5"
          aria-hidden
        />
      </div>

      <div className="font-mono text-[11px] text-fg-subtle mt-3 truncate" title={project.path}>
        {project.path}
      </div>
    </article>
  );
}
