fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    let proto_files = &[
        "proto/health.proto",
        "proto/common.proto",
        "proto/workspace.proto",
        "proto/config.proto",
        "proto/task.proto",
        "proto/thread.proto",
        "proto/label.proto",
        "proto/integration.proto",
        "proto/roadmap.proto",
        "proto/provider.proto",
        "proto/project.proto",
        "proto/agent.proto",
        "proto/event.proto",
        "proto/run.proto",
        "proto/conversation.proto",
        "proto/snapshot.proto",
        "proto/log.proto",
        "proto/diff.proto",
        "proto/wiki.proto",
        "proto/usage.proto",
    ];
    tonic_build::configure()
        .file_descriptor_set_path(out_dir.join("modula_v1_descriptor.bin"))
        .compile_protos(proto_files, &["proto"])?;
    Ok(())
}
