import type { Project } from "../../../types";
import { call } from "../invoke";
import type {
  CloneProjectBody,
  CommitsResponse,
  CreateProjectBody,
  DiffTextResponse,
  NumstatBlock,
  RepoBranches,
  UpdateProjectBody,
  WorkingDiff,
} from "../types";

export class ProjectResource {
  all(ws: string) {
    return call<Project[]>("project_list", { workspaceId: ws });
  }

  get(ws: string, id: string) {
    return call<Project>("project_get", { workspaceId: ws, projectId: id });
  }

  create(ws: string, body: CreateProjectBody) {
    return call<{ id: string }>("project_create", {
      workspaceId: ws,
      name: body.name,
      path: body.path,
      baseBranch: body.base_branch,
    });
  }

  clone(ws: string, body: CloneProjectBody) {
    return call<{ id: string }>("project_clone", {
      workspaceId: ws,
      name: body.name,
      path: body.path,
      gitUrl: body.git_url,
    });
  }

  update(ws: string, id: string, body: UpdateProjectBody) {
    return call<void>("project_update", {
      workspaceId: ws,
      projectId: id,
      path: body.path,
      baseBranch: body.base_branch,
    });
  }

  delete(ws: string, id: string) {
    return call<void>("project_delete", { workspaceId: ws, projectId: id });
  }

  repoBranches(ws: string, path: string) {
    return call<RepoBranches>("project_repo_branches", { workspaceId: ws, path });
  }

  diff(ws: string, id: string, branch?: string) {
    return call<WorkingDiff>("project_diff", { workspaceId: ws, projectId: id, branch });
  }

  diffText(ws: string, id: string, branch?: string) {
    return call<DiffTextResponse>("project_diff_text", { workspaceId: ws, projectId: id, branch });
  }

  commits(ws: string, id: string, opts: { branch?: string; since?: string } = {}) {
    return call<CommitsResponse>("project_commits", {
      workspaceId: ws,
      projectId: id,
      branch: opts.branch,
      since: opts.since,
    });
  }

  commitDiff(ws: string, id: string, sha: string, branch?: string) {
    return call<NumstatBlock>("project_commit_diff", {
      workspaceId: ws,
      projectId: id,
      sha,
      branch,
    });
  }

  stage(ws: string, id: string, files: string[], branch?: string) {
    return call<void>("project_stage", { workspaceId: ws, projectId: id, files, branch });
  }

  unstage(ws: string, id: string, files: string[], branch?: string) {
    return call<void>("project_unstage", { workspaceId: ws, projectId: id, files, branch });
  }
}
