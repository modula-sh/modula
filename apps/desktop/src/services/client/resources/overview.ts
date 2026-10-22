import type { OverviewWorkspace } from "../../../types";
import { call } from "../invoke";

export class OverviewResource {
  get() {
    return call<OverviewWorkspace[]>("overview_get");
  }
}
