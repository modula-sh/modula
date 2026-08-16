import { call } from "../invoke";
import type { ToolStatus } from "../types";

export class SystemResource {
  tools() {
    return call<ToolStatus[]>("system_tools");
  }
}
