use std::pin::Pin;

use modula_rpc::json::{json_to_struct, struct_to_json};
use modula_rpc::v1::{
    event_service_server::EventService, CreateEventRequest, CreateEventResponse, Event,
    ListEventsRequest, ListEventsResponse, ListRecentEventsRequest, ListRecentEventsResponse,
    WatchEventsRequest, WorkspaceEvent,
};
use serde_json::Value as Json;
use tokio::sync::broadcast::error::RecvError;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use super::error::to_status;
use crate::state::AppState;
use modula_db::events::EventRecord;

pub struct EventHandler {
    pub state: AppState,
}

type EventStream = Pin<Box<dyn Stream<Item = Result<WorkspaceEvent, Status>> + Send>>;

fn row_to_proto(r: EventRecord) -> Event {
    let data = json_to_struct(r.data_json());
    Event {
        id: r.id,
        r#type: r.type_,
        data,
        processed: r.processed,
        created_at: r.created_at,
    }
}

#[tonic::async_trait]
impl EventService for EventHandler {
    async fn list(
        &self,
        req: Request<ListEventsRequest>,
    ) -> Result<Response<ListEventsResponse>, Status> {
        let ws = req.into_inner().workspace_id;
        let rows = self.state.events.list(&ws).await.map_err(to_status)?;
        Ok(Response::new(ListEventsResponse {
            events: rows.into_iter().map(row_to_proto).collect(),
        }))
    }

    async fn create(
        &self,
        req: Request<CreateEventRequest>,
    ) -> Result<Response<CreateEventResponse>, Status> {
        let body = req.into_inner();
        let data = body.data.map(struct_to_json).unwrap_or(Json::Null);
        let id = self
            .state
            .events
            .publish(&body.workspace_id, &body.r#type, data)
            .await
            .map_err(to_status)?;
        Ok(Response::new(CreateEventResponse { id }))
    }

    async fn list_recent_events(
        &self,
        req: Request<ListRecentEventsRequest>,
    ) -> Result<Response<ListRecentEventsResponse>, Status> {
        let body = req.into_inner();
        let events = self
            .state
            .events
            .list_recent(&body.workspace_id, body.limit, body.after_seq)
            .await
            .map_err(to_status)?;
        Ok(Response::new(ListRecentEventsResponse {
            events: events.into_iter().map(WorkspaceEvent::from).collect(),
        }))
    }

    type WatchStream = EventStream;
    async fn watch(
        &self,
        req: Request<WatchEventsRequest>,
    ) -> Result<Response<Self::WatchStream>, Status> {
        let body = req.into_inner();
        self.state
            .workspaces
            .get(&body.workspace_id)
            .await
            .map_err(to_status)?;
        let mut rx = self.state.bus.subscribe(&body.workspace_id).await;
        let after_seq = body.after_seq;
        let stream = async_stream::try_stream! {
            loop {
                match rx.recv().await {
                    Ok(ev) => {
                        let seq = ev.seq as i64;
                        if after_seq != 0 && seq <= after_seq {
                            continue;
                        }
                        if let Some(we) = modula_types::WorkspaceEvent::from_parts(
                            seq, &ev.workspace_id, "", &ev.type_, &ev.data,
                        ) {
                            yield WorkspaceEvent::from(we);
                        }
                    }
                    Err(RecvError::Lagged(n)) => {
                        tracing::warn!("[event.watch] subscriber lagged, skipped {n} events");
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }
}
