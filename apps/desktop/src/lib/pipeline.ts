import type { PipelineStatus, PipelineTone, Snapshot } from "../types";

/** Pull the pipeline list out of a snapshot, defaulting to []. */
export function getPipeline(snap: Snapshot | null | undefined): PipelineStatus[] {
  const raw = snap?.config?.pipeline;
  return Array.isArray(raw) ? (raw as PipelineStatus[]) : [];
}

/** Lookup helper: status key → its config entry. Returns null if the status
 * isn't defined in the pipeline (rare; usually means data drift). */
export function pipelineStatusFor(
  pipeline: PipelineStatus[],
  status: string | null | undefined,
): PipelineStatus | null {
  if (!status) return null;
  return pipeline.find((p) => p.key === status) ?? null;
}

export function pipelineTone(
  pipeline: PipelineStatus[],
  status: string | null | undefined,
): PipelineTone {
  return pipelineStatusFor(pipeline, status)?.tone ?? "zinc";
}

export function pipelineLabel(
  pipeline: PipelineStatus[],
  status: string | null | undefined,
): string {
  return pipelineStatusFor(pipeline, status)?.label ?? status ?? "";
}

/** Tailwind classes for a small status dot. One shade per tone — readable on
 * both light and dark backgrounds without looking washed out. */
export const toneDotClasses: Record<PipelineTone, string> = {
  zinc: "bg-zinc-400",
  green: "bg-green-500",
  yellow: "bg-yellow-500",
  red: "bg-red-500",
  blue: "bg-blue-500",
  purple: "bg-purple-500",
  orange: "bg-orange-500",
};
