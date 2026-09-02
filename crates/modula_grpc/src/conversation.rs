use std::pin::Pin;

use modula_rpc::json::{json_to_struct, struct_to_json};
use modula_rpc::v1::{
    conv_event, conversation_service_server::ConversationService, AttachConversationRequest,
    CancelConversationRequest, CancelConversationResponse, ConvEvent, Conversation,
    ConversationSummary, CreateConversationRequest, CreateConversationResponse,
    DeleteConversationRequest, DeleteConversationResponse, DeltaEvent, DequeueMessageRequest,
    DoneEvent, EnqueueMessageRequest, ErrorEvent, GetConversationRequest, ListConversationsRequest,
    ListConversationsResponse, QueuedMessage, QueuedMessagesResponse, SendMessageRequest,
    SessionEvent, ToolUseEvent, UpdateConversationRequest, UpdateConversationResponse,
};
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use super::error::to_status;
use modula_services::conversations::{self, ConvStream, WireEvent};
use modula_state::AppState;

pub struct ConversationHandler {
    pub state: AppState,
}

type ConvEventStream = Pin<Box<dyn Stream<Item = Result<ConvEvent, Status>> + Send>>;

/// Single decode point from the internal `WireEvent` vocabulary to the typed
/// `ConvEvent` proto, shared by `Send` and `Attach` so they cannot drift.
fn to_conv_event(event: WireEvent) -> ConvEvent {
    let inner = match event {
        WireEvent::Session { id } => conv_event::Event::Session(SessionEvent { id }),
        WireEvent::ToolUse { name, input } => conv_event::Event::ToolUse(ToolUseEvent {
            name,
            input: json_to_struct(input),
        }),
        WireEvent::Delta { text } => conv_event::Event::Delta(DeltaEvent { text }),
        WireEvent::Done => conv_event::Event::Done(DoneEvent {}),
        WireEvent::Error { message } => conv_event::Event::Error(ErrorEvent { message }),
    };
    ConvEvent { event: Some(inner) }
}

/// Build a per-client server stream from a transport-agnostic [`ConvStream`].
/// The replay buffer is flushed first, then live events until a terminal one.
/// Dropping the returned stream drops the receiver, which detaches the client
/// without cancelling the underlying run.
fn into_stream((initial, mut rx): ConvStream) -> ConvEventStream {
    let stream = async_stream::try_stream! {
        for event in initial {
            let terminal = event.is_terminal();
            yield to_conv_event(event);
            if terminal {
                return;
            }
        }
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let terminal = event.is_terminal();
                    yield to_conv_event(event);
                    if terminal {
                        break;
                    }
                }
                Err(RecvError::Closed) => break,
                // Slow consumer: deltas were dropped — the client heals by
                // refetching the persisted turn, but log the gap so it's visible.
                Err(RecvError::Lagged(n)) => {
                    tracing::warn!("conversation stream lagged, dropped {n} events");
                    continue;
                }
            }
        }
    };
    Box::pin(stream)
}

#[tonic::async_trait]
impl ConversationService for ConversationHandler {
    async fn list(
        &self,
        req: Request<ListConversationsRequest>,
    ) -> Result<Response<ListConversationsResponse>, Status> {
        let ws = req.into_inner().workspace_id;
        let rows = self
            .state
            .conversations
            .list(&ws)
            .await
            .map_err(to_status)?;
        let running = self.state.conv_runs.running(&ws).await;
        let conversations = rows
            .into_iter()
            .map(|r| ConversationSummary {
                running: running.contains(&r.id),
                id: r.id,
                title: r.title,
                provider_id: r.provider_id,
                model: r.model,
                context: json_to_struct(r.context),
                queued: r.queued.into_iter().map(QueuedMessage::from).collect(),
                updated_at: r.updated_at,
            })
            .collect();
        Ok(Response::new(ListConversationsResponse { conversations }))
    }

    async fn get(
        &self,
        req: Request<GetConversationRequest>,
    ) -> Result<Response<Conversation>, Status> {
        let body = req.into_inner();
        let conv = self
            .state
            .conversations
            .get(&body.workspace_id, &body.conversation_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(Conversation::from(conv)))
    }

    async fn create(
        &self,
        req: Request<CreateConversationRequest>,
    ) -> Result<Response<CreateConversationResponse>, Status> {
        let body = req.into_inner();
        let context = body.context.map(struct_to_json).unwrap_or(json!({}));
        let id = self
            .state
            .conversations
            .create(
                &body.workspace_id,
                body.title,
                &body.provider_id,
                body.model,
                context,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(CreateConversationResponse { id }))
    }

    async fn update(
        &self,
        req: Request<UpdateConversationRequest>,
    ) -> Result<Response<UpdateConversationResponse>, Status> {
        let body = req.into_inner();
        self.state
            .conversations
            .update(&body.workspace_id, &body.conversation_id, body.title)
            .await
            .map_err(to_status)?;
        Ok(Response::new(UpdateConversationResponse { ok: true }))
    }

    async fn delete(
        &self,
        req: Request<DeleteConversationRequest>,
    ) -> Result<Response<DeleteConversationResponse>, Status> {
        let body = req.into_inner();
        // Best-effort: wind down any in-flight run so the background task stops
        // writing to the row we're about to delete.
        let _ = conversations::cancel(
            self.state.conv.clone(),
            body.workspace_id.clone(),
            body.conversation_id.clone(),
        )
        .await;
        self.state
            .conversations
            .delete(&body.workspace_id, &body.conversation_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(DeleteConversationResponse { ok: true }))
    }

    type SendStream = ConvEventStream;
    async fn send(
        &self,
        req: Request<SendMessageRequest>,
    ) -> Result<Response<Self::SendStream>, Status> {
        let body = req.into_inner();
        if body.message.is_empty() {
            return Err(Status::invalid_argument("message is required"));
        }
        let handle = conversations::open_send(
            self.state.conv.clone(),
            body.workspace_id,
            body.conversation_id,
            body.message,
            body.model,
        )
        .await
        .map_err(to_status)?;
        Ok(Response::new(into_stream(handle)))
    }

    type AttachStream = ConvEventStream;
    async fn attach(
        &self,
        req: Request<AttachConversationRequest>,
    ) -> Result<Response<Self::AttachStream>, Status> {
        let body = req.into_inner();
        let handle = conversations::open_attach(
            self.state.conv.clone(),
            body.workspace_id,
            body.conversation_id,
        )
        .await;
        Ok(Response::new(into_stream(handle)))
    }

    async fn cancel(
        &self,
        req: Request<CancelConversationRequest>,
    ) -> Result<Response<CancelConversationResponse>, Status> {
        let body = req.into_inner();
        conversations::cancel(
            self.state.conv.clone(),
            body.workspace_id,
            body.conversation_id,
        )
        .await
        .map_err(to_status)?;
        Ok(Response::new(CancelConversationResponse { ok: true }))
    }

    async fn enqueue(
        &self,
        req: Request<EnqueueMessageRequest>,
    ) -> Result<Response<QueuedMessagesResponse>, Status> {
        let body = req.into_inner();
        let queued = self
            .state
            .conversations
            .enqueue(&body.workspace_id, &body.conversation_id, &body.message)
            .await
            .map_err(to_status)?;
        // Nothing running means the message goes now rather than parking — the
        // client's idea of "still streaming" is always slightly stale.
        let conv = self.state.conv.clone();
        tokio::spawn(conversations::drain_queue(
            conv,
            body.workspace_id,
            body.conversation_id,
        ));
        Ok(Response::new(QueuedMessagesResponse {
            queued: queued.into_iter().map(QueuedMessage::from).collect(),
        }))
    }

    async fn dequeue(
        &self,
        req: Request<DequeueMessageRequest>,
    ) -> Result<Response<QueuedMessagesResponse>, Status> {
        let body = req.into_inner();
        let queued = self
            .state
            .conversations
            .dequeue(&body.workspace_id, &body.conversation_id, &body.queued_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(QueuedMessagesResponse {
            queued: queued.into_iter().map(QueuedMessage::from).collect(),
        }))
    }
}
