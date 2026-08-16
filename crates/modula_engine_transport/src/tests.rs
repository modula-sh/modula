use std::path::PathBuf;
use std::sync::Mutex;

#[cfg(unix)]
use crate::LocalListener;
use crate::{EngineEndpoint, LocalIpcEndpoint};

// `MODULA_ENGINE_SOCKET` is process-global, so the tests that mutate it must not
// run concurrently or one's `remove_var` races another's read.
static ENV_LOCK: Mutex<()> = Mutex::new(());

// ─── Unit: endpoint resolution order ─────────────────────────────────────────

#[test]
fn resolve_uses_explicit_path() {
    let explicit = PathBuf::from("/tmp/test.sock");
    let ep = LocalIpcEndpoint::resolve(Some(explicit.clone())).unwrap();
    assert_eq!(ep.path(), explicit.as_path());
}

#[test]
fn resolve_falls_back_to_env() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::set_var("MODULA_ENGINE_SOCKET", "/tmp/env.sock");
    let ep = LocalIpcEndpoint::resolve(None).unwrap();
    std::env::remove_var("MODULA_ENGINE_SOCKET");

    assert_eq!(ep.path(), PathBuf::from("/tmp/env.sock").as_path());
}

#[test]
fn resolve_explicit_overrides_env() {
    let _guard = ENV_LOCK.lock().unwrap();
    std::env::set_var("MODULA_ENGINE_SOCKET", "/tmp/env.sock");
    let explicit = PathBuf::from("/tmp/explicit.sock");
    let ep = LocalIpcEndpoint::resolve(Some(explicit.clone())).unwrap();
    std::env::remove_var("MODULA_ENGINE_SOCKET");
    assert_eq!(ep.path(), explicit.as_path());
}

#[test]
fn engine_endpoint_display() {
    let ipc = EngineEndpoint::LocalIpc(LocalIpcEndpoint::new("/tmp/engine.sock"));
    assert_eq!(ipc.to_string(), "ipc:/tmp/engine.sock");
}

// ─── Integration smoke: HealthService round-trip over a per-test IPC ─────────

#[cfg(unix)]
#[tokio::test]
async fn health_check_round_trip_unix() {
    use modula_rpc::v1::{
        health_service_client::HealthServiceClient,
        health_service_server::{HealthService, HealthServiceServer},
        HealthCheckRequest, HealthCheckResponse, HealthStatus,
    };
    use tonic::{Request, Response, Status};

    struct Svc;
    #[tonic::async_trait]
    impl HealthService for Svc {
        async fn check(
            &self,
            _req: Request<HealthCheckRequest>,
        ) -> Result<Response<HealthCheckResponse>, Status> {
            Ok(Response::new(HealthCheckResponse {
                status: HealthStatus::Serving as i32,
                pid: std::process::id(),
            }))
        }
    }

    // The bind path's parent dir must be owned by our UID (see ipc_security),
    // so the socket lives in a tempdir, not directly in /tmp.
    let sock_dir = tempfile::TempDir::new().unwrap();
    let sock_path = sock_dir.path().join("test.sock");

    let endpoint = LocalIpcEndpoint::new(sock_path.clone());
    let listener = LocalListener::bind(&endpoint).await.unwrap();

    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(HealthServiceServer::new(Svc))
            .serve_with_incoming(listener.incoming())
            .await
            .unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let channel = endpoint.connect().await.unwrap();
    let mut client = HealthServiceClient::new(channel);
    let resp = client
        .check(HealthCheckRequest {
            service: String::new(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.status, HealthStatus::Serving as i32);

    server.abort();
    let _ = std::fs::remove_file(&sock_path);
}

// ─── Unit: stale socket detect + reclaim (Unix only) ─────────────────────────

#[cfg(unix)]
#[tokio::test]
async fn stale_socket_is_reclaimed() {
    use std::os::unix::net::UnixListener as StdListener;

    let sock_dir = tempfile::TempDir::new().unwrap();
    let path = sock_dir.path().join("test.sock");

    let _std_listener = StdListener::bind(&path).unwrap();
    drop(_std_listener);

    assert!(path.exists(), "stale socket file should exist");

    let ep = LocalIpcEndpoint::new(path.clone());
    crate::stale::handle_stale(&ep).await.unwrap();

    assert!(!path.exists(), "stale socket should be removed");
}

#[cfg(unix)]
#[tokio::test]
async fn live_engine_is_refused() {
    use modula_rpc::v1::{
        health_service_server::{HealthService, HealthServiceServer},
        HealthCheckRequest, HealthCheckResponse, HealthStatus,
    };
    use tonic::{Request, Response, Status};

    struct Svc;
    #[tonic::async_trait]
    impl HealthService for Svc {
        async fn check(
            &self,
            _: Request<HealthCheckRequest>,
        ) -> Result<Response<HealthCheckResponse>, Status> {
            Ok(Response::new(HealthCheckResponse {
                status: HealthStatus::Serving as i32,
                pid: std::process::id(),
            }))
        }
    }

    // The bind path's parent dir must be owned by our UID (see ipc_security),
    // so the socket lives in a tempdir, not directly in /tmp.
    let sock_dir = tempfile::TempDir::new().unwrap();
    let sock_path = sock_dir.path().join("test.sock");

    let endpoint = LocalIpcEndpoint::new(sock_path.clone());
    let listener = LocalListener::bind(&endpoint).await.unwrap();

    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(HealthServiceServer::new(Svc))
            .serve_with_incoming(listener.incoming())
            .await
            .ok();
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let ep2 = LocalIpcEndpoint::new(sock_path.clone());
    let err = crate::stale::handle_stale(&ep2).await.unwrap_err();
    assert!(
        matches!(err, crate::Error::AlreadyRunning(_)),
        "expected AlreadyRunning, got: {err}"
    );

    server.abort();
    let _ = std::fs::remove_file(&sock_path);
}
