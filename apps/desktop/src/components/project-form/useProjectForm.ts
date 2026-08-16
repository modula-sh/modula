import { useCallback, useEffect, useState } from "react";
import { client } from "../../services/client";
import type { Project } from "../../types";
import type { ProjectFormState } from "./ProjectFields";
import { useRepoBranches } from "./useRepoBranches";

function emptyFormState(): ProjectFormState {
  return { name: "", path: "", base_branch: "main", mode: "existing", git_url: "" };
}

function formStateFrom(p: Project): ProjectFormState {
  return {
    name: p.name,
    path: p.path,
    base_branch: p.base_branch ?? "main",
    mode: "existing",
    git_url: "",
  };
}

/** Form state for the shared project fields. Pass an existing project to edit,
 * or `null` to create. Validity reflects the fields the active mode requires.
 * `branches` drives the base-branch dropdown, fetched from the selected path. */
export function useProjectForm(ws: string, detail: Project | null) {
  const isCreate = detail === null;
  const [state, setState] = useState<ProjectFormState>(() =>
    detail ? formStateFrom(detail) : emptyFormState(),
  );

  useEffect(() => {
    setState(detail ? formStateFrom(detail) : emptyFormState());
  }, [detail]);

  const patch = useCallback(<K extends keyof ProjectFormState>(k: K, v: ProjectFormState[K]) => {
    setState((s) => ({ ...s, [k]: v }));
  }, []);

  const repo = useRepoBranches(ws, state.path);

  // Keep base_branch valid: preserve a still-present selection, else pick the default.
  useEffect(() => {
    if (repo.loading || repo.branches.length === 0) return;
    setState((s) =>
      repo.branches.includes(s.base_branch)
        ? s
        : { ...s, base_branch: repo.defaultBranch ?? repo.branches[0] },
    );
  }, [repo.loading, repo.branches, repo.defaultBranch]);

  const branches = {
    options: repo.branches,
    loading: repo.loading,
    isGit: repo.isGit,
    disabled: !state.path.trim() || repo.loading || !repo.isGit || repo.branches.length === 0,
  };

  const valid =
    isCreate && state.mode === "clone"
      ? !!state.name.trim() && !!state.path.trim() && !!state.git_url.trim()
      : (isCreate ? !!state.name.trim() : true) &&
        !!state.path.trim() &&
        !!state.base_branch.trim();

  return { state, patch, isCreate, valid, branches };
}

export function createProject(ws: string, state: ProjectFormState): Promise<{ id: string }> {
  if (state.mode === "clone") {
    return client.project.clone(ws, {
      name: state.name.trim(),
      path: state.path.trim(),
      git_url: state.git_url.trim(),
    });
  }
  return client.project.create(ws, {
    name: state.name.trim(),
    path: state.path.trim(),
    base_branch: state.base_branch.trim(),
  });
}

export async function updateProject(
  ws: string,
  id: string,
  state: ProjectFormState,
): Promise<void> {
  await client.project.update(ws, id, {
    path: state.path.trim(),
    base_branch: state.base_branch.trim(),
  });
}
