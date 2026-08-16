//! Agent run log tailing. Each `agent_runs` row carries its `log_path`
//! (basename inside `<ws>/logs/`); clients tail it as a stream of lines. The
//! tail follows the file until the consumer drops the stream (detach) — the
//! underlying run is never affected.

use std::path::PathBuf;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom};
use tokio_stream::wrappers::IntervalStream;
use tokio_stream::{Stream, StreamExt};

use super::workspaces::WorkspaceService;
use crate::core::error::{ApiError, ApiResult};

/// Log-path resolution service. DIs [`WorkspaceService`] to locate the
/// workspace's `logs/` directory; `tail_lines` (the actual streaming) stays an
/// agnostic fs helper the handler drives.
#[derive(Clone)]
pub struct LogsService {
    workspaces: WorkspaceService,
}

impl LogsService {
    pub fn new(workspaces: WorkspaceService) -> Self {
        Self { workspaces }
    }

    /// Validate a log basename and resolve it to a path under `<ws>/logs/`.
    /// Rejects path traversal and non-`.log` names.
    pub async fn resolve_path(&self, ws: &str, name: &str) -> ApiResult<PathBuf> {
        if name.contains('/') || name.contains("..") || !name.ends_with(".log") {
            return Err(ApiError::BadRequest("invalid log name".into()));
        }
        let ws_dir = self.workspaces.workspace_dir(ws).await?;
        let path = ws_dir.join("logs").join(name);
        if !path.exists() {
            return Err(ApiError::NotFound("log not found".into()));
        }
        Ok(path)
    }
}

/// Drain the file's existing content, then tail new appends, yielding each
/// non-empty line with its trailing newline stripped. The stream follows the
/// file indefinitely; it ends only when the file can't be read or the consumer
/// drops it.
pub fn tail_lines(path: PathBuf) -> impl Stream<Item = String> {
    async_stream::stream! {
        let Ok(file) = tokio::fs::File::open(&path).await else {
            return;
        };
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        // Drain existing content, then poll for appends on a fixed interval.
        let mut ticks = IntervalStream::new(tokio::time::interval(Duration::from_millis(500)));
        loop {
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim_end_matches('\n');
                        if !trimmed.is_empty() {
                            yield trimmed.to_string();
                        }
                    }
                    Err(_) => return,
                }
            }
            if ticks.next().await.is_none() {
                return;
            }
            // No-op on tokio File, kept so a truncate/rotate is observed.
            let _ = reader.seek(SeekFrom::Current(0)).await;
        }
    }
}
