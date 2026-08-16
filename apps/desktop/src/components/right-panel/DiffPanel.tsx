import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Minus, Plus } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useRef } from "react";
import { useRightPanel } from "../../contexts/RightPanelProvider";
import { projectKeys, useDiffText } from "../../queries/project";
import { client } from "../../services/client";
import { Spinner } from "../Spinner";
import type { FilePatch, FilePatchAction } from "./FilePatchRow";
import { FileSection } from "./FileSection";

type Group = "staged" | "unstaged" | "untracked";

const GROUPS: { key: Group; label: string }[] = [
  { key: "staged", label: "Staged" },
  { key: "unstaged", label: "Unstaged" },
  { key: "untracked", label: "Untracked" },
];

export function DiffPanel({
  workspace,
  project,
  branch,
  focusFile,
  focusGroup,
}: {
  workspace: string;
  project: string;
  branch?: string;
  focusFile?: string;
  focusGroup?: Group;
}) {
  const { data } = useDiffText(workspace, project, branch);
  const queryClient = useQueryClient();
  const fileRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const { setTitle } = useRightPanel();
  // Track whether we've already scrolled to focusFile so polling refreshes
  // don't keep yanking the user back to the original anchor.
  const scrolledRef = useRef(false);

  useEffect(() => {
    scrolledRef.current = false;
  }, [focusFile, focusGroup, workspace, project, branch]);

  const stageMutation = useMutation({
    mutationFn: ({ leaf, files }: { leaf: "stage" | "unstage"; files: string[] }) =>
      leaf === "stage"
        ? client.project.stage(workspace, project, files, branch)
        : client.project.unstage(workspace, project, files, branch),
    onSettled: () =>
      queryClient.invalidateQueries({ queryKey: projectKeys.diffText(workspace, project, branch) }),
  });

  useEffect(() => {
    if (!data?.branch) return;
    setTitle(
      <>
        <span className="text-fg-subtle">Diff</span> {data.branch}
      </>,
    );
  }, [data, setTitle]);

  useLayoutEffect(() => {
    if (!data || !focusFile || scrolledRef.current) return;
    const key = `${focusGroup ?? ""}::${focusFile}`;
    const el = fileRefs.current.get(key);
    if (!el) return;
    scrolledRef.current = true;
    el.scrollIntoView({ behavior: "auto", block: "start" });
  }, [data, focusFile, focusGroup]);

  const postFiles = useCallback(
    (leaf: "stage" | "unstage", files: string[]) => stageMutation.mutate({ leaf, files }),
    [stageMutation],
  );

  const LEAF_FOR: Record<Group, "stage" | "unstage"> = {
    staged: "unstage",
    unstaged: "stage",
    untracked: "stage",
  };

  const buildAction = (g: Group, paths: string[], titleSuffix: string): FilePatchAction => {
    const leaf = LEAF_FOR[g];
    return {
      icon: leaf === "stage" ? <Plus size={12} /> : <Minus size={12} />,
      title: `${leaf === "stage" ? "Stage" : "Unstage"} ${titleSuffix}`,
      onClick: () => postFiles(leaf, paths),
    };
  };

  const fileActionFor = (g: Group) => (f: FilePatch) => buildAction(g, [f.path], "file");
  const headerActionFor = (g: Group, files: FilePatch[]) =>
    files.length === 0
      ? undefined
      : buildAction(
          g,
          files.map((f) => f.path),
          "all in this section",
        );

  if (!data) {
    return (
      <div className="flex-1 flex items-center justify-center p-6">
        <Spinner size={24} />
      </div>
    );
  }

  const empty = GROUPS.every((g) => data[g.key].length === 0);
  if (empty) {
    return <div className="p-6 text-fg-subtle text-xs">No changes.</div>;
  }

  return (
    <div>
      {GROUPS.map((g) => (
        <FileSection
          key={g.key}
          header={g.label}
          headerAction={headerActionFor(g.key, data[g.key])}
          files={data[g.key]}
          fileRefs={fileRefs}
          refKeyFor={(p) => `${g.key}::${p}`}
          autoOpenPath={focusGroup === g.key ? focusFile : undefined}
          actionFor={fileActionFor(g.key)}
        />
      ))}
    </div>
  );
}
