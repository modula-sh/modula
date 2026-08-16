import type { Snapshot } from "../../../types";
import { call } from "../invoke";

export class SnapshotResource {
  get(ws: string) {
    return call<Snapshot>("snapshot_get", { workspaceId: ws });
  }
}
