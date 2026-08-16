// Public entry point for the engine API client module. Consumers import from
// `services/client`; internals (http transport, per-resource classes) stay
// private to this folder.

export { client, errorMessage } from "./client";
export * from "./types";
