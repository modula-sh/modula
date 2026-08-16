use std::time::Duration;

use crate::{Error, LocalIpcEndpoint};

/// Handles a potentially stale or live socket/pipe at the endpoint path.
///
/// - If the path does not exist: no-op (clean state).
/// - If a live engine answers a `HealthService.Check`: returns `Error::AlreadyRunning`.
/// - If the path exists but no live engine answers: removes the stale file and returns `Ok(())`.
pub async fn handle_stale(endpoint: &LocalIpcEndpoint) -> Result<(), Error> {
    if !endpoint.path().exists() {
        return Ok(());
    }

    if probe_live(endpoint).await {
        return Err(Error::AlreadyRunning(endpoint.to_string()));
    }

    #[cfg(unix)]
    std::fs::remove_file(endpoint.path())?;

    Ok(())
}

/// Attempts a `HealthService.Check` RPC with a short timeout.
/// Returns `true` only if a live engine responds successfully.
async fn probe_live(endpoint: &LocalIpcEndpoint) -> bool {
    use modula_rpc::v1::{health_service_client::HealthServiceClient, HealthCheckRequest};

    let channel = match tokio::time::timeout(Duration::from_millis(300), endpoint.connect()).await {
        Ok(Ok(ch)) => ch,
        _ => return false,
    };

    let mut client = HealthServiceClient::new(channel);
    let call = client.check(HealthCheckRequest {
        service: String::new(),
    });
    matches!(
        tokio::time::timeout(Duration::from_millis(300), call).await,
        Ok(Ok(_))
    )
}
