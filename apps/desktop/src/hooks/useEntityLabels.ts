import { useMemo } from "react";
import { useSnapshot } from "../contexts/SnapshotContext";

/** Flat `id → display label` map built from the current workspace snapshot.
 * Use this anywhere a UUID-shaped identifier needs to be rendered as a name
 * (breadcrumbs, headers, picker labels). Falls back to the id at the call
 * site when an entry is missing. */
export function useEntityLabels(): Record<string, string> {
  const { snap } = useSnapshot();
  return useMemo(() => {
    const out: Record<string, string> = {};
    if (!snap) return out;
    snap.config?.projects?.forEach((p) => {
      out[p.id] = p.name;
    });
    snap.config?.providers?.forEach((p) => {
      out[p.id] = p.name;
    });
    snap.config?.agents?.forEach((a) => {
      out[a.id] = a.name;
    });
    snap.tasks?.forEach((t) => {
      out[t.id] = t.title;
    });
    return out;
  }, [snap]);
}
