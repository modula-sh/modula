import type { ExternalItem, Integration } from "../../../types";
import { call } from "../invoke";

export class IntegrationResource {
  all(ws: string) {
    return call<Integration[]>("integration_list", { workspaceId: ws });
  }

  /** Health-checks the connection before the engine persists it. */
  connect(ws: string, id: string, data: Record<string, unknown>) {
    return call<void>("integration_connect", { workspaceId: ws, id, data });
  }

  remove(ws: string, id: string) {
    return call<void>("integration_delete", { workspaceId: ws, id });
  }

  /** `params` is per-request config merged over the stored data (github repo). */
  search(ws: string, id: string, query: string, params: Record<string, unknown>) {
    return call<ExternalItem[]>("integration_search", { workspaceId: ws, id, query, params });
  }

  fetchItem(ws: string, id: string, key: string, params: Record<string, unknown>) {
    return call<ExternalItem>("integration_fetch", { workspaceId: ws, id, key, params });
  }

  /** `owner/repo` names selectable for the integration (github only). */
  repos(ws: string, id: string) {
    return call<string[]>("integration_repos", { workspaceId: ws, id });
  }
}
