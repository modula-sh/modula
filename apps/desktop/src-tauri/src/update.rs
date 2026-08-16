//! In-app updater, owned by the Rust/Tauri layer (not the frontend) so the
//! mechanism stays independent of the UI. A background task polls GitHub
//! Releases on a build-time interval; the frontend is a thin view that reads
//! [`update_status`] on mount, follows the [`STATUS_EVENT`], and triggers
//! [`install_update`] / [`restart_app`]. Download progress streams over a
//! per-install [`Channel`].

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{async_runtime, AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use time::format_description::well_known::Rfc3339;
use url::Url;

/// Poll interval, baked by `build.rs` from `app-settings.toml`.
fn check_interval() -> Duration {
    let secs: u64 = env!("MODULA_UPDATE_INTERVAL_SECS")
        .parse()
        .unwrap_or(21_600);
    Duration::from_secs(secs.max(1))
}

/// The `latest.json` endpoint, derived from `[update].repo` (the value flipped
/// at public launch).
fn endpoint() -> Result<Url, String> {
    let url = format!(
        "https://github.com/{}/releases/latest/download/latest.json",
        env!("MODULA_UPDATE_REPO")
    );
    Url::parse(&url).map_err(|e| format!("invalid updater endpoint {url}: {e}"))
}

/// Emitted on every state transition so the UI follows what the Tauri layer is
/// doing, including a background check that finds an update.
const STATUS_EVENT: &str = "update://status";

/// The updater's coarse state. Serializes to the lowercase strings the frontend
/// switches on (e.g. `Available` -> `"available"`).
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateState {
    Idle,
    Checking,
    Available,
    Downloading,
    Downloaded,
    Error,
}

/// Coarse updater state, shaped for direct display by the frontend. Download
/// progress is not here — it streams over the install [`Channel`].
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub state: UpdateState,
    pub version: Option<String>,
    /// Release date, RFC3339.
    pub date: Option<String>,
    /// One-line release notes.
    pub notes: Option<String>,
    pub error: Option<String>,
}

impl UpdateStatus {
    fn new(state: UpdateState) -> Self {
        Self {
            state,
            version: None,
            date: None,
            notes: None,
            error: None,
        }
    }

    fn from_update(state: UpdateState, update: &Update) -> Self {
        Self {
            state,
            version: Some(update.version.clone()),
            date: update.date.and_then(|d| d.format(&Rfc3339).ok()),
            notes: update.body.clone(),
            error: None,
        }
    }
}

/// Pending [`Update`] handle (kept between check and install) plus the latest status.
pub struct UpdaterState {
    pending: Mutex<Option<Update>>,
    status: Mutex<UpdateStatus>,
}

impl UpdaterState {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(None),
            status: Mutex::new(UpdateStatus::new(UpdateState::Idle)),
        }
    }
}

/// Download progress streamed to the frontend over the install [`Channel`].
#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        content_length: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        chunk_length: usize,
    },
    Finished,
}

/// Store the status and notify the frontend in one step.
fn emit_status(app: &AppHandle, status: UpdateStatus) {
    if let Some(state) = app.try_state::<UpdaterState>() {
        *state.status.lock().unwrap() = status.clone();
    }
    let _ = app.emit(STATUS_EVENT, status);
}

/// Spawn the background poll loop.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    async_runtime::spawn(async move {
        let interval = check_interval();
        loop {
            run_check(&app).await;
            tokio::time::sleep(interval).await;
        }
    });
}

async fn run_check(app: &AppHandle) {
    // Skip while something is already found or in flight, to avoid clobbering it
    // (and flickering the card via a transient `checking`).
    {
        let state = app.state::<UpdaterState>();
        let current = state.status.lock().unwrap().state;
        if !matches!(current, UpdateState::Idle | UpdateState::Error) {
            return;
        }
    }
    emit_status(app, UpdateStatus::new(UpdateState::Checking));

    match check(app).await {
        Ok(Some(update)) => {
            let status = UpdateStatus::from_update(UpdateState::Available, &update);
            *app.state::<UpdaterState>().pending.lock().unwrap() = Some(update);
            emit_status(app, status);
        }
        Ok(None) => emit_status(app, UpdateStatus::new(UpdateState::Idle)),
        Err(e) => {
            // A read-only check failure must never surface; log and stay idle.
            eprintln!("modula: update check failed: {e}");
            emit_status(app, UpdateStatus::new(UpdateState::Idle));
        }
    }
}

async fn check(app: &AppHandle) -> Result<Option<Update>, String> {
    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint()?])
        .and_then(|b| b.build())
        .map_err(|e| e.to_string())?;
    updater.check().await.map_err(|e| e.to_string())
}

/// Snapshot read by the frontend on mount to sync with the Tauri layer.
#[tauri::command]
pub fn update_status(state: State<'_, UpdaterState>) -> UpdateStatus {
    state.status.lock().unwrap().clone()
}

/// Download + install the pending update (signature-verified by the plugin),
/// streaming progress over `on_event`. User-triggered only — no silent update.
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    state: State<'_, UpdaterState>,
    on_event: Channel<DownloadEvent>,
) -> Result<(), String> {
    let Some(update) = state.pending.lock().unwrap().take() else {
        return Err("no update available".to_string());
    };

    emit_status(
        &app,
        UpdateStatus::from_update(UpdateState::Downloading, &update),
    );

    let mut started = false;
    let on_finish = on_event.clone();
    let result = update
        .download_and_install(
            move |chunk_length, content_length| {
                if !started {
                    started = true;
                    let _ = on_event.send(DownloadEvent::Started { content_length });
                }
                let _ = on_event.send(DownloadEvent::Progress { chunk_length });
            },
            move || {
                let _ = on_finish.send(DownloadEvent::Finished);
            },
        )
        .await;

    match result {
        Ok(()) => {
            emit_status(
                &app,
                UpdateStatus::from_update(UpdateState::Downloaded, &update),
            );
            Ok(())
        }
        Err(e) => {
            let message = e.to_string();
            // Keep the metadata (card stays as "Retry") and restore the handle.
            let mut status = UpdateStatus::from_update(UpdateState::Error, &update);
            status.error = Some(message.clone());
            *state.pending.lock().unwrap() = Some(update);
            emit_status(&app, status);
            Err(message)
        }
    }
}

/// Relaunch into the installed update (tauri core restart). User-triggered only.
#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.restart();
}
