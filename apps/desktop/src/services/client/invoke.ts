// Request core for the engine's gRPC bridge. Every unary engine call goes
// through a Tauri `invoke` command in `src-tauri/src/grpc/*`; those commands
// dial the local IPC socket, call the generated gRPC client, and map the
// response into the JSON shapes `types.ts` models (see `grpc/dto.rs`). A failing
// command rejects with its `Err(String)`, which `errorMessage` renders.

import { invoke } from "@tauri-apps/api/core";

/** Invoke a gRPC bridge command. `args` keys are camelCase (Tauri maps them to
 * the command's snake_case parameters). */
export function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}

/** Extract a human-readable message from any thrown value. Bridge commands
 * reject with a plain string; other failures may be `Error` or exotic. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
