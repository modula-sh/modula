# Plugins

A plugin is a trusted first-party crate compiled into the engine binary — no
sandbox, no ABI boundary, no IPC. It contributes schema, gRPC services, CLI
subcommands and background work through one entrypoint.

Seam: [`crates/modula_plugin`](../crates/modula_plugin/src/lib.rs). Crate graph:
[`ARCHITECTURE.md`](ARCHITECTURE.md#crates).

## Traits

A plugin **is** its components. One type implements whichever seams it needs;
there is no separate definition object and runtime object.

| Trait | Required methods | Fills |
|:------|:-----------------|:------|
| `Plugin` | `metadata() -> PluginMetadata`<br>`register(self: Arc<Self>, &mut PluginRegistry)` | Identity, and the claim on the seams below. |
| `PluginService` | none — all defaulted | State and background work. |
| `PluginGrpc` | `add_to(Router) -> Router` | gRPC services on the engine's router. |
| `PluginCli` | `names() -> &[&str]`<br>`command(Command) -> Command`<br>`run(&ModulaClient, &ArgMatches)` | `modula` subcommands. Runs in the CLI process, not the engine. |

`PluginMetadata` is `{ name, version, description }`; `name` is the registry key
and must be unique.

## Registration

`register` receives the `Arc` the registry will hold, so a plugin hands clones
of *itself* to each seam it fills:

```rust
#[derive(Default)]
pub struct BillingPlugin(OnceLock<State>);

impl Plugin for BillingPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "billing",
            version: env!("CARGO_PKG_VERSION"),
            description: "Usage metering and invoicing",
        }
    }

    fn register(self: Arc<Self>, registry: &mut PluginRegistry) {
        registry
            .migrations(db::migrator)
            .cli(self.clone())
            .service(self.clone())
            .grpc(self);
    }
}

#[async_trait] impl PluginService for BillingPlugin { /* init, start, shutdown */ }
impl PluginGrpc for BillingPlugin { /* add_to */ }
#[async_trait] impl PluginCli for BillingPlugin { /* names, command, run */ }
```

The composition root calls `registry.register(BillingPlugin::default())`. Every
builder method is optional — register only the seams you implement. A plugin
with no gRPC surface simply never calls `.grpc(...)`.

Because all three seams are the same object, state built in `init` is visible to
the gRPC handlers and the background task with no sharing machinery: hold it in
a `OnceLock` field and read it from `&self`.

`migrations` takes the constructor (`fn() -> Migrator`), not a `Migrator`:
`Migrator` is not `Clone` and `PluginRegistry` is.

## Lifecycle

| Phase | Call | State |
|:------|:-----|:------|
| 1. compose | `Plugin::register` | No database yet. Claim seams only. |
| 2. migrate | `PluginRegistry::migrate` | Core schema applied; each registered migrator runs in registration order. |
| 3. init | `PluginService::init(ctx)` | DB open and migrated. Build runtime state here. |
| 4. serve | `PluginGrpc::add_to(router)` | Router assembled; plugin services added last, so they cannot shadow a core one. |
| 5. start | `PluginService::start` | Listener bound — a plugin may now call back into the engine over IPC. |
| 6. stop | `PluginService::shutdown` | Not yet wired; see *Known gaps*. |

`init` receives a `PluginContext`:

| Field | |
|:------|:--|
| `db: Database` | The migrated pool. |
| `bus: Bus` | Live event delivery; the durable record is the `events` table. |
| `modula_dir: PathBuf` | `~/.modula`, the root for any file the plugin owns. |
| `engine_socket: String` | This engine's IPC endpoint. `ctx.client()` dials it. |

It is deliberately small: `modula-db` repositories are stateless structs a
plugin constructs itself. Plugin state lives in the plugin behind its own
interior mutability, never on the engine's `AppState`.

## Constraints

- **Migration versions are global.** All plugins and the core set share one
  `_sqlx_migrations` table. Numbers are allocated once and never renumbered — a
  changed checksum breaks existing databases. Migrators run with
  `ignore_missing`, so each tolerates versions it does not own.
- **A plugin may alter core tables; only the plugin may read those columns.**
  `0004_sync.sql` adds `workspace_settings.db_epoch` and the accessor lives in
  the plugin. Core code naming a plugin-added column compiles, then fails at
  runtime in a build without that plugin.
- **CLI names must not shadow a core command.** `PluginCli::names` is matched
  before the engine's own parse.
- **Tests must go through `register`.** Build a fixture DB with
  `PluginRegistry::migrate`, not the plugin's `migrator()` directly, so a
  forgotten `.migrations(...)` fails in tests rather than at runtime.

## Build modes

One binary, one workspace. What differs between an open-source and an official
build is a plugin crate's *implementation*, not the set of crates:
`plugins/remote` is always a workspace member and always registered.

| Build | `plugins/remote` holds | `AVAILABLE` |
|:------|:-----------------------|:------------|
| open source | a `NotImplemented` stub | `false` |
| official | the real crate, copied from `../modula-plugins` | `true` |

```bash
cargo build --workspace       # stub
./scripts/dev-plugins.sh      # swap, then dev.sh
./scripts/build-plugins.sh    # swap, then build.sh
./scripts/reset-plugins.sh    # back to the stub
```

`swap-plugins.sh` copies over tracked files and leaves the tree dirty by
design. `reset-plugins.sh` restores from git — **commit stub changes before
swapping**, or they are lost.

`../modula-plugins` is private and `remote` additionally needs
`../modula-shared`. Neither is independently buildable: their path dependencies
are written relative to `modula/plugins/<name>`, where the swap puts them.

**Why a stub rather than a cargo feature.** Two independent reasons:

- Cargo resolves optional path dependencies' manifests regardless of feature
  state, so `optional = true` on a proprietary crate still breaks a bare clone.
- The desktop cannot make the dependency conditional at all:
  `tauri::generate_handler!` must name every command in one expansion,
  `Builder::invoke_handler` may be called only once, and `Invoke` is not
  `Clone`, so two handlers cannot be chained.

A crate that is always present and sometimes inert satisfies both.

**The stub's public API must mirror the real crate's.** The desktop compiles
against whichever is present and deserializes `types::*` in the webview, so a
drifted field breaks only the official build, and only once someone runs
`swap-plugins.sh`. Keep the surface small.

## Known gaps

- `PluginService::shutdown` is defined and implemented but never invoked; the
  engine's shutdown path does not call it.
