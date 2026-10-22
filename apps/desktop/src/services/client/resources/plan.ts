import type { Plan } from "../../../types";
import { call } from "../invoke";

export class PlanResource {
  get(ws: string, taskId: string, variantId: string) {
    return call<Plan>("plan_get", { workspaceId: ws, taskId, variantId });
  }
}
