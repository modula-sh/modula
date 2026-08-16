use anyhow::Result;
use modula_rpc::v1::{
    CreateWorkspaceRequest, GetConfigRequest, HealthCheckRequest, HealthStatus,
    ListWorkspacesRequest,
};
use modula_test_support::Harness;
use tonic::Code;

#[tokio::test]
async fn workspace_lifecycle() -> Result<()> {
    let h = Harness::start().await?;

    // Health reports SERVING over the IPC channel.
    let status = h
        .health()
        .check(HealthCheckRequest {
            service: String::new(),
        })
        .await?
        .into_inner()
        .status;
    assert_eq!(status, HealthStatus::Serving as i32);

    // Empty workspaces list at boot.
    let list = h
        .workspaces()
        .list(ListWorkspacesRequest {})
        .await?
        .into_inner()
        .workspaces;
    assert!(list.is_empty());

    // Create with name → returns UUID id.
    let created = h
        .workspaces()
        .create(CreateWorkspaceRequest {
            name: "demo".into(),
            description: None,
        })
        .await?
        .into_inner();
    assert_eq!(created.name, "demo");
    assert!(!created.id.is_empty());
    let ws_id = created.id;

    // List now contains demo.
    let list = h
        .workspaces()
        .list(ListWorkspacesRequest {})
        .await?
        .into_inner()
        .workspaces;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, ws_id);
    assert_eq!(list[0].name, "demo");

    // Missing name → InvalidArgument.
    let err = h
        .workspaces()
        .create(CreateWorkspaceRequest {
            name: String::new(),
            description: None,
        })
        .await
        .expect_err("empty name must be rejected");
    assert_eq!(err.code(), Code::InvalidArgument);

    // Config reads the seeded YAML.
    let cfg = h
        .config()
        .get(GetConfigRequest {
            workspace_id: ws_id.clone(),
        })
        .await?
        .into_inner();
    assert!(!cfg.pipeline.is_empty());

    // Unknown workspace → NotFound on config.
    let err = h
        .config()
        .get(GetConfigRequest {
            workspace_id: "00000000-0000-0000-0000-000000000000".into(),
        })
        .await
        .expect_err("unknown workspace must 404");
    assert_eq!(err.code(), Code::NotFound);

    Ok(())
}
