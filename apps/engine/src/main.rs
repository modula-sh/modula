//! Thin entrypoint; `lib.rs` owns the composition.
fn main() -> anyhow::Result<()> {
    modula_engine::run()
}
