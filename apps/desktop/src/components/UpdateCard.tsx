import { RotateCw } from "lucide-react";
import type { AppUpdate } from "../hooks/useAppUpdate";
import { Button } from "./Button";
import { Pill } from "./Pill";
import { Spinner } from "./Spinner";

/** Sidebar "Update Now" card. Renders only when the Tauri layer has surfaced an
 * update; mounted between the Chats nav and the Settings block. Two in-place
 * actions: download + install, then relaunch — nothing runs without a click. */
export function UpdateCard({ state, version, progress, install, restart }: AppUpdate) {
  // Nothing to act on (idle / mid-check): hide the card.
  if (!version || state === "idle" || state === "checking") return null;

  return (
    <div className="shrink-0 mx-2 mb-2 px-3 py-2 bg-card rounded-md border border-border/60 shadow-card space-y-2">
      <div className="flex items-center justify-center gap-2">
        <span className="text-[13px] font-medium text-fg">Update Available</span>
        <Pill size="sm" variant="flat">
          {version}
        </Pill>
      </div>
      {state === "downloaded" ? (
        <Button className="w-full justify-center" onClick={restart}>
          <RotateCw size={13} />
          Restart now
        </Button>
      ) : state === "downloading" ? (
        <Button className="w-full justify-center" disabled>
          <Spinner size={13} />
          {progress === null ? "Downloading…" : `${Math.round(progress * 100)}%`}
        </Button>
      ) : (
        <Button className="w-full justify-center" onClick={install}>
          {state === "error" ? "Retry update" : "Update Now"}
        </Button>
      )}
    </div>
  );
}
