import { GitPullRequest, GitPullRequestCreate } from "lucide-react";
import { Button } from "./Button";
import { openUrl } from "./openUrl";

export function PrLink({ createUrl, prUrl }: { createUrl: string | null; prUrl: string | null }) {
  if (prUrl) {
    return (
      <Button onClick={() => openUrl(prUrl)} className="!py-0.5 !text-[10px]">
        <GitPullRequest size={12} />
        PR
      </Button>
    );
  }
  if (createUrl) {
    return (
      <Button onClick={() => openUrl(createUrl)} className="!py-0.5 !text-[10px]">
        <GitPullRequestCreate size={12} />
        Create PR
      </Button>
    );
  }
  return null;
}
