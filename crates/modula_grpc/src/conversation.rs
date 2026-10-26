use std::pin::Pin;

use modula_rpc::json::{json_to_struct, struct_to_json};
use modula_rpc::v1::{
    conv_event, conversation_service_server::ConversationService, terminal_output,
    AttachConversationRequest, CancelConversationRequest, CancelConversationResponse, ConvEvent,
    Conversation, ConversationSummary, CreateConversationRequest, CreateConversationResponse,
    DeleteConversationRequest, DeleteConversationResponse, DeltaEvent, DequeueMessageRequest,
    DoneEvent, EnqueueMessageRequest, ErrorEvent, ForkConversationRequest, GetConversationRequest,
    LaunchSpec, ListConversationsRequest, ListConversationsResponse, ListMessagesRequest,
    ListMessagesResponse, QueuedMessage, QueuedMessagesResponse, SendMessageRequest, SessionEvent,
    TerminalAck, TerminalLaunchRequest, TerminalOpenRequest, TerminalOutput, TerminalResizeRequest,
    TerminalWriteRequest, ThinkingEvent, ToolResultEvent, ToolUseEvent, UpdateConversationRequest,
    UpdateConversationResponse, UserEvent,
};
use serde_json::json;
use tokio::sync::broadcast::error::RecvError;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use super::error::to_status;
use modula_services::conversations::{self, ConvStream, WireEvent};
use modula_services::terminals::{self, TermEvent};
use modula_state::AppState;

pub struct ConversationHandler {
    pub state: AppState,
}

type ConvEventStream = Pin<Box<dyn Stream<Item = Result<ConvEvent, Status>> + Send>>;
type TerminalStream = Pin<Box<dyn Stream<Item = Result<TerminalOutput, Status>> + Send>>;

/// PTY sizes are u16; anything larger is clamped rather than refused.
fn dim(n: u32) -> u16 {
    n.clamp(1, u16::MAX as u32) as u16
}

/// Single decode point from the internal `WireEvent` vocabulary to the typed
/// `ConvEvent` proto, shared by `Send` and `Attach` so they cannot drift.
fn to_conv_event(event: WireEvent) -> ConvEvent {
    let inner = match event {
        WireEvent::Session { id } => conv_event::Event::Session(SessionEvent { id }),
        WireEvent::ToolUse {
            id,
            name,
            input,
            parent_id,
        } => conv_event::Event::ToolUse(ToolUseEvent {
            name,
            input: json_to_struct(input),
            id,
            parent_id,
        }),
        WireEvent::ToolResult {
            id,
            output,
            is_error,
            output_truncated,
        } => conv_event::Event::ToolResult(ToolResultEvent {
            id,
            output,
            is_error,
            output_truncated,
        }),
        WireEvent::Delta { text } => conv_event::Event::Delta(DeltaEvent { text }),
        WireEvent::Thinking { text } => conv_event::Event::Thinking(ThinkingEvent { text }),
        WireEvent::User { text } => conv_event::Event::User(UserEvent { text }),
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
                mode: r.mode,
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
            .get(
                &body.workspace_id,
                &body.conversation_id,
                body.message_limit,
            )
            .await
            .map_err(to_status)?;
        let mut out = Conversation::from(conv);
        out.running = self
            .state
            .conv_runs
            .running(&body.workspace_id)
            .await
            .contains(&out.id);
        Ok(Response::new(out))
    }

    async fn list_messages(
        &self,
        req: Request<ListMessagesRequest>,
    ) -> Result<Response<ListMessagesResponse>, Status> {
        let body = req.into_inner();
        let messages = self
            .state
            .conversations
            .messages(
                &body.workspace_id,
                &body.conversation_id,
                Some(body.before_seq),
                Some(body.limit),
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(ListMessagesResponse {
            messages: messages.into_iter().map(Into::into).collect(),
        }))
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
                body.mode.as_deref(),
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(CreateConversationResponse { id }))
    }

    async fn fork(
        &self,
        req: Request<ForkConversationRequest>,
    ) -> Result<Response<CreateConversationResponse>, Status> {
        let body = req.into_inner();
        let id = self
            .state
            .conversations
            .fork(&body.workspace_id, &body.conversation_id, body.through_seq)
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

    async fn terminal_launch(
        &self,
        req: Request<TerminalLaunchRequest>,
    ) -> Result<Response<LaunchSpec>, Status> {
        let body = req.into_inner();
        let spec = conversations::terminal_launch(
            &self.state.conv,
            &body.workspace_id,
            &body.conversation_id,
            body.prompt.as_deref(),
        )
        .await
        .map_err(to_status)?;
        Ok(Response::new(spec.into()))
    }

    type TerminalOpenStream = TerminalStream;
    async fn terminal_open(
        &self,
        req: Request<TerminalOpenRequest>,
    ) -> Result<Response<Self::TerminalOpenStream>, Status> {
        let body = req.into_inner();
        let mut rx = terminals::open(
            &self.state.conv,
            &body.workspace_id,
            &body.conversation_id,
            body.prompt.as_deref(),
            dim(body.cols),
            dim(body.rows),
        )
        .await
        .map_err(to_status)?;
        let stream = async_stream::stream! {
            while let Some(event) = rx.recv().await {
                let event = match event {
                    TermEvent::Data(data) => terminal_output::Event::Data(data),
                    TermEvent::Exit(code) => terminal_output::Event::ExitCode(code),
                };
                yield Ok(TerminalOutput { event: Some(event) });
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }

    async fn terminal_write(
        &self,
        req: Request<TerminalWriteRequest>,
    ) -> Result<Response<TerminalAck>, Status> {
        let body = req.into_inner();
        self.state
            .conv
            .terminals
            .write(&body.workspace_id, &body.conversation_id, &body.data)
            .await
            .map_err(to_status)?;
        Ok(Response::new(TerminalAck {}))
    }

    async fn terminal_resize(
        &self,
        req: Request<TerminalResizeRequest>,
    ) -> Result<Response<TerminalAck>, Status> {
        let body = req.into_inner();
        self.state
            .conv
            .terminals
            .resize(
                &body.workspace_id,
                &body.conversation_id,
                dim(body.cols),
                dim(body.rows),
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(TerminalAck {}))
    }
}
