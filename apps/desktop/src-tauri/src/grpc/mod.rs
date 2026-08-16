//! The Tauri ↔ engine bridge. The single [`ModulaClient`](modula_client::ModulaClient)
//! is held as Tauri managed state; each `invoke` command calls a client method,
//! which converts protos to `modula-types` domain types at the edge, and returns
//! the domain type for the webview to consume. Streaming commands forward the
//! client's domain-item streams onto a Tauri `Channel`, detaching on teardown
//! (see [`log`]).

// Command modules stay `pub(crate)` so `tauri::generate_handler!` in `lib.rs`
// can reach each command at its full path (e.g. `grpc::task::task_list`). A
// `pub use` re-export of the function alone would not bring along the sibling
// items `#[tauri::command]` generates, so the handler macro needs the real path.
pub(crate) mod agent;
pub(crate) mod config;
pub(crate) mod conversation;
pub(crate) mod diff;
pub(crate) mod event;
pub(crate) mod health;
pub(crate) mod integration;
pub(crate) mod label;
pub(crate) mod log;
pub(crate) mod project;
pub(crate) mod provider;
pub(crate) mod roadmap;
pub(crate) mod run;
pub(crate) mod snapshot;
pub(crate) mod system;
pub(crate) mod task;
pub(crate) mod thread;
pub(crate) mod usage;
pub(crate) mod wiki;
pub(crate) mod workspace;
