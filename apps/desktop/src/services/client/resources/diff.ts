import type { VariantDiffs, VariantPr } from "../../../types";
import { call } from "../invoke";

export class DiffResource {
  variant(ws: string, taskId: string, variantId: string) {
    return call<VariantDiffs>("variant_diff", { workspaceId: ws, taskId, variantId });
  }

  variantPr(ws: string, taskId: string, variantId: string) {
    return call<VariantPr>("variant_pr", { workspaceId: ws, taskId, variantId });
  }
}
