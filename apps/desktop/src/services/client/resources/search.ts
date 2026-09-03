import type { SearchHit, SearchKind } from "../../../types";
import { call } from "../invoke";

export class SearchResource {
  /** `kinds` empty means every kind; `limit` 0 the engine's per-kind default. */
  query(ws: string, q: string, opts?: { kinds?: SearchKind[]; limit?: number }) {
    return call<SearchHit[]>("search_query", {
      workspaceId: ws,
      query: q,
      kinds: opts?.kinds ?? [],
      limit: opts?.limit ?? 0,
    });
  }
}
