import { type QueryClient, useQueryClient } from "@tanstack/react-query";
import { Channel, invoke } from "@tauri-apps/api/core";
import { useEffect, useRef } from "react";
import { useStreamApi } from "../contexts/ConversationStreamProvider";
import { snapshotKeys } from "../queries/snapshot";

/** A workspace event from the engine's `EventService.Watch` stream, forwarded
 * by the `event_watch` Tauri command (shape: `grpc/dto.rs::workspace_event`).
 * Only the routing fields are modelled; each event also carries `seq`,
 * `workspace_id`, and `created_at`. */
interface EngineEvent {
  type: string;
  task_id?: string;
  conversation_id?: string;
  running?: boolean;
}

// Route a stream event to the cached queries it invalidates. The workspace
// snapshot aggregates tasks, roadmap, agents, runs, config, and conversations,
// so every event refreshes it (replacing the old snapshot SSE poll). The key
// prefixes mirror the factories in `queries/{thread,conversation}.ts` and the
// `["tasks", ws, …]` detail keys in `queries/task.ts`.
function invalidate(qc: QueryClient, ws: string, ev: EngineEvent) {
  qc.invalidateQueries({ queryKey: snapshotKeys.all(ws) });
  if (ev.type.startsWith("thread_") && ev.task_id) {
    qc.invalidateQueries({ queryKey: ["threads", ws, ev.task_id] });
  } else if (ev.type.startsWith("conversation_")) {
    qc.invalidateQueries({ queryKey: ["conversations", ws] });
  } else if (
    ev.type.startsWith("task_") ||
    ev.type === "variant_updated" ||
    ev.type === "agent_run"
  ) {
    qc.invalidateQueries({ queryKey: ["tasks", ws] });
  } else if (ev.type.startsWith("agent_")) {
    qc.invalidateQueries({ queryKey: ["agents", ws] });
  } else if (ev.type.startsWith("provider_")) {
    qc.invalidateQueries({ queryKey: ["providers", ws] });
  }
}

/** Subscribe to the engine's live workspace event stream and drive TanStack
 * Query invalidation off it, replacing timer-polling for event-driven data.
 * Re-subscribes whenever the workspace changes; dropping the channel on cleanup
 * detaches the watch on the backend. */
export function useEngineEvents(workspace: string) {
  const qc = useQueryClient();
  const streams = useStreamApi();
  // Read through a ref so a new `api` identity does not tear down the watch.
  const streamsRef = useRef(streams);
  streamsRef.current = streams;
  useEffect(() => {
    if (!workspace) return;
    const channel = new Channel<EngineEvent>();
    channel.onmessage = (ev) => {
      invalidate(qc, workspace, ev);
      // A turn started somewhere else — another window, or a paired phone.
      // Attaching on mount alone misses it, and misses it permanently.
      if (ev.type === "conversation_run" && ev.running && ev.conversation_id) {
        streamsRef.current?.attach(workspace, ev.conversation_id);
      }
    };
    invoke("event_watch", { workspaceId: workspace, afterSeq: 0, onEvent: channel }).catch(() => {
      // Watch ended (workspace switch, engine restart). A remount re-subscribes.
    });
    return () => {
      channel.onmessage = () => {};
    };
  }, [workspace, qc]);
}
