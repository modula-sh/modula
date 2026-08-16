use modula_rpc::v1::{HealthCheckRequest, HealthStatus};

use crate::ModulaClient;

impl ModulaClient {
    /// Whether the engine is serving on the IPC endpoint right now. A connect
    /// failure (nothing listening) or a non-`SERVING` status both read as down.
    pub async fn is_serving(&self) -> bool {
        let Ok(mut client) = self.health().await else {
            return false;
        };
        client
            .check(HealthCheckRequest {
                service: String::new(),
            })
            .await
            .map(|r| r.into_inner().status == HealthStatus::Serving as i32)
            .unwrap_or(false)
    }

    /// OS pid of the serving engine; `None` when nothing is serving or the
    /// engine predates the `pid` health field.
    pub async fn serving_pid(&self) -> Option<u32> {
        let Ok(mut client) = self.health().await else {
            return None;
        };
        let resp = client
            .check(HealthCheckRequest {
                service: String::new(),
            })
            .await
            .ok()?
            .into_inner();
        (resp.status == HealthStatus::Serving as i32 && resp.pid != 0).then_some(resp.pid)
    }
}
