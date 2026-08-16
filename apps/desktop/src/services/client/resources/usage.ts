import type { UsageRecord } from "../../../types";
import { call } from "../invoke";

export class UsageResource {
  all(ws: string) {
    return call<UsageRecord[]>("usage_get", { workspaceId: ws });
  }
}
