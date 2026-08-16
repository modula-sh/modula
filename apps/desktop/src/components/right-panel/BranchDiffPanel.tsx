import { useEffect, useRef } from "react";
import { useRightPanel } from "../../contexts/RightPanelProvider";
import { useVariantDiff, useVariantPr } from "../../queries/diff";
import { PrLink } from "../PrLink";
import { Spinner } from "../Spinner";
import { FileSection } from "./FileSection";

// "PR view": full branch-vs-base diff for every project the variant touches.
// Reuses FilePatchRow; one section per project.
export function BranchDiffPanel({
  workspace,
  task,
  variant,
}: {
  workspace: string;
  task: string;
  variant: string;
}) {
  const { data } = useVariantDiff(workspace, task, variant);
  const { data: pr } = useVariantPr(workspace, task, variant);
  const fileRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const { setTitle } = useRightPanel();

  useEffect(() => {
    const branch = data?.projects[0]?.branch;
    if (!branch) return;
    setTitle(
      <>
        <span className="text-fg-subtle">Diff</span> {branch}
      </>,
    );
  }, [data, setTitle]);

  if (!data) {
    return (
      <div className="flex-1 flex items-center justify-center p-6">
        <Spinner size={24} />
      </div>
    );
  }

  if (data.projects.length === 0) {
    return <div className="p-6 text-fg-subtle text-xs">No changes on this branch.</div>;
  }

  return (
    <div>
      {data.projects.map((p) => {
        const projectPr = pr?.projects.find((x) => x.name === p.name);
        return (
          <FileSection
            key={p.name}
            header={
              <>
                {p.name} <span className="text-green-500">+{p.insertions}</span>{" "}
                <span className="text-red-500">−{p.deletions}</span>
              </>
            }
            headerExtra={
              projectPr && <PrLink createUrl={projectPr.create_url} prUrl={projectPr.pr_url} />
            }
            files={p.patches}
            fileRefs={fileRefs}
            refKeyFor={(path) => `${p.name}::${path}`}
          />
        );
      })}
    </div>
  );
}
