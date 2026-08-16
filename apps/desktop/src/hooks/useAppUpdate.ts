import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";

export type UpdateState =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "downloaded"
  | "error";

/** Coarse updater state from the Rust layer (shared by the `update_status`
 * command and the `update://status` event). */
interface UpdateStatus {
  state: UpdateState;
  version: string | null;
  date: string | null;
  notes: string | null;
  error: string | null;
}

/** Per-install download progress streamed over a Tauri channel by `install_update`. */
type DownloadEvent =
  | { event: "Started"; data: { contentLength: number | null } }
  | { event: "Progress"; data: { chunkLength: number } }
  | { event: "Finished" };

export interface AppUpdate {
  state: UpdateState;
  /** Non-null once a newer release is found; drives whether the card renders. */
  version: string | null;
  /** Release date (RFC3339) when the manifest provided one. */
  date: string | null;
  /** Optional one-line release notes. */
  notes: string | null;
  /** 0..1 while downloading when the size is known, else null. */
  progress: number | null;
  /** Download + install the available update. Only call on explicit user action. */
  install: () => void;
  /** Relaunch into the installed update. Only call on explicit user action. */
  restart: () => void;
}

const INITIAL: UpdateStatus = {
  state: "idle",
  version: null,
  date: null,
  notes: null,
  error: null,
};

/** Thin view over the Rust-owned updater: reflects its state and forwards user
 * intent. Syncs on mount via `update_status`, then follows `update://status` —
 * so the card reflects a background check that finds an update on its own. */
export function useAppUpdate(): AppUpdate {
  const [status, setStatus] = useState<UpdateStatus>(INITIAL);
  const [progress, setProgress] = useState<number | null>(null);
  // Prevents a second install while one is in flight.
  const busyRef = useRef(false);

  useEffect(() => {
    invoke<UpdateStatus>("update_status")
      .then(setStatus)
      .catch(() => {
        // No updater in this context (e.g. non-Tauri host) — stay idle.
      });
    const unlisten = listen<UpdateStatus>("update://status", (e) => {
      setStatus(e.payload);
      // Progress is owned by the install channel; clear it outside a download.
      if (e.payload.state !== "downloading") setProgress(null);
    });
    return () => {
      unlisten.then((off) => off());
    };
  }, []);

  const install = useCallback(() => {
    if (busyRef.current) return;
    busyRef.current = true;
    setProgress(0);
    let total = 0;
    let received = 0;
    const channel = new Channel<DownloadEvent>();
    channel.onmessage = (msg) => {
      if (msg.event === "Started") {
        total = msg.data.contentLength ?? 0;
      } else if (msg.event === "Progress") {
        received += msg.data.chunkLength;
        setProgress(total ? received / total : null);
      } else if (msg.event === "Finished") {
        setProgress(1);
      }
    };
    invoke("install_update", { onEvent: channel })
      .catch(() => {
        // The "error" state arrives via the status event; nothing to do here.
      })
      .finally(() => {
        busyRef.current = false;
      });
  }, []);

  const restart = useCallback(() => {
    invoke("restart_app").catch(() => {
      // If relaunch fails the app keeps running in the "downloaded" state.
    });
  }, []);

  return {
    state: status.state,
    version: status.version,
    date: status.date,
    notes: status.notes,
    progress,
    install,
    restart,
  };
}
