import { Button } from "../Button";
import { DropdownSelect } from "../DropdownMenu";
import { FieldRow } from "../FieldRow";
import { FileInput } from "../FileInput";
import { TextInput } from "../TextInput";

export interface ProjectFormState {
  name: string;
  path: string;
  base_branch: string;
  mode: "existing" | "clone";
  git_url: string;
}

export interface BranchSelectState {
  options: string[];
  loading: boolean;
  isGit: boolean;
  disabled: boolean;
}

function branchPlaceholder(path: string, b: BranchSelectState): string {
  if (!path.trim()) return "Select a folder first";
  if (b.loading) return "Loading branches…";
  if (!b.isGit) return "Not a git repository";
  if (b.options.length === 0) return "No branches found";
  return "Select a branch";
}

/** Name / path / base-branch inputs for a project, shared by the in-app editor
 * and onboarding. In create mode the name is editable; in edit mode it's fixed. */
export function ProjectFields({
  state,
  onChange,
  isCreate,
  branches,
  autoFocus = false,
  className = "",
}: {
  state: ProjectFormState;
  onChange: <K extends keyof ProjectFormState>(key: K, value: ProjectFormState[K]) => void;
  isCreate: boolean;
  branches: BranchSelectState;
  autoFocus?: boolean;
  className?: string;
}) {
  const clone = isCreate && state.mode === "clone";
  return (
    <section className={`border border-card-border/50 bg-card rounded-xl px-3 ${className}`.trim()}>
      {isCreate && (
        <FieldRow label="source" description="Use an existing checkout or clone a remote repo.">
          <div className="flex gap-1">
            <Button tone="tab" active={!clone} onClick={() => onChange("mode", "existing")}>
              Existing
            </Button>
            <Button tone="tab" active={clone} onClick={() => onChange("mode", "clone")}>
              Clone
            </Button>
          </div>
        </FieldRow>
      )}
      <FieldRow label="name" description="Display name shown in the dashboard.">
        {isCreate ? (
          <TextInput
            value={state.name}
            onChange={(e) => onChange("name", e.target.value)}
            placeholder="Display name"
            padded
            className="w-full"
            autoFocus={autoFocus}
          />
        ) : (
          <span className="text-fg">{state.name}</span>
        )}
      </FieldRow>
      {clone && (
        <FieldRow label="git url" description="Remote repo to clone (https or ssh).">
          <TextInput
            value={state.git_url}
            onChange={(e) => onChange("git_url", e.target.value)}
            placeholder="https://github.com/org/repo.git or git@github.com:org/repo.git"
            mono
            padded
            className="w-full"
          />
        </FieldRow>
      )}
      <FieldRow
        label="path"
        description={
          clone ? "Absolute path to clone into." : "Absolute filesystem path to the git checkout."
        }
      >
        <FileInput
          value={state.path}
          onChange={(v) => onChange("path", v)}
          directory
          placeholder="/absolute/path/to/repo"
          mono
          padded
          className="w-full"
        />
      </FieldRow>
      {!clone && (
        <FieldRow label="base branch" description="Branch that variants branch from (e.g. main).">
          <DropdownSelect
            variant="field"
            mono
            padded
            className="w-full"
            value={state.base_branch}
            onChange={(v) => onChange("base_branch", v)}
            disabled={branches.disabled}
            placeholder={branchPlaceholder(state.path, branches)}
            options={branches.options.map((b) => ({ value: b, label: b }))}
          />
        </FieldRow>
      )}
    </section>
  );
}
