import { createContext, useContext } from "react";
import type { PipelineStatus } from "../types";

/** Pipeline (ordered roadmap statuses + display metadata) sourced from the
 * `pipeline` config block (rows in `pipeline_statuses`). Provided at the App
 * level once the snapshot arrives; before then, descendants see an empty list
 * and fall back to raw status strings. */
export const PipelineContext = createContext<PipelineStatus[]>([]);

export function usePipeline(): PipelineStatus[] {
  return useContext(PipelineContext);
}
