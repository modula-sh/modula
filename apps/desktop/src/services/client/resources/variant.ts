import { call } from "../invoke";

export class VariantResource {
  setStatus(ws: string, taskId: string, variantId: string, status: string) {
    return call<void>("variant_update", { workspaceId: ws, taskId, variantId, status });
  }
}
