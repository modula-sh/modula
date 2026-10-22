//! Conversation calls. Unary CRUD plus the streaming `send`/`attach` methods,
//! which yield a domain [`ConvEvent`] stream. Dropping a `send`/`attach` stream
//! detaches without cancelling the underlying run (use `cancel_conversation`
//! for that), preserving the MOD-003 detach-without-cancel semantics.

use modula_rpc::json::json_to_struct;
use modula_rpc::v1::{
    AttachConversationRequest, CancelConversationRequest, ConvEvent as PbConvEvent,
    CreateConversationRequest, DeleteConversationRequest, DequeueMessageRequest,
    EnqueueMessageRequest, ForkConversationRequest, GetConversationRequest, ListMessagesRequest,
    SendMessageRequest, UpdateConversationRequest,
};
use modula_types::{ChatMessage, ConvEvent, Conversation, QueuedMessage};
use serde_json::Value;
use tokio_stream::{Stream, StreamExt};

use crate::error::{rpc, ClientError};
use crate::ModulaClient;

/// Map a raw `ConvEvent` proto stream to domain [`ConvEvent`]s — shared by
/// `send` and `attach`, which differ only in how the stream is opened.
fn conv_event_stream(
    stream: tonic::Streaming<PbConvEvent>,
) -> impl Stream<Item = Result<ConvEvent, ClientError>> {
    stream.map(|item| item.map(ConvEvent::from).map_err(rpc))
}

impl ModulaClient {
    /// The conversation with its newest `message_limit` messages, or all of them.
    pub async fn get_conversation(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        message_limit: Option<u32>,
    ) -> Result<Conversation, ClientError> {
        let resp = self
            .conversations()
            .await?
            .get(GetConversationRequest {
                workspace_id: workspace_id.to_string(),
                conversation_id: conversation_id.to_string(),
                message_limit,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(Conversation::from(resp))
    }

    /// The `limit` messages before `before_seq`, oldest first.
    pub async fn conversation_messages(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        before_seq: u32,
        limit: u32,
    ) -> Result<Vec<ChatMessage>, ClientError> {
        let resp = self
            .conversations()
            .await?
            .list_messages(ListMessagesRequest {
                workspace_id: workspace_id.to_string(),
                conversation_id: conversation_id.to_string(),
                before_seq,
                limit,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.messages.into_iter().map(ChatMessage::from).collect())
    }

    /// Create a conversation; returns the new conversation id.
    pub async fn create_conversation(
        &self,
        workspace_id: &str,
        provider_id: &str,
        title: Option<String>,
        model: Option<String>,
        context: Option<Value>,
    ) -> Result<String, ClientError> {
        let resp = self
            .conversations()
            .await?
            .create(CreateConversationRequest {
                workspace_id: workspace_id.to_string(),
                provider_id: provider_id.to_string(),
                title,
                model,
                context: context.and_then(json_to_struct),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.id)
    }

    /// Clone a conversation through `through_seq`; returns the clone's id.
    pub async fn fork_conversation(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        through_seq: Option<u32>,
    ) -> Result<String, ClientError> {
        let resp = self
            .conversations()
            .await?
            .fork(ForkConversationRequest {
                workspace_id: workspace_id.to_string(),
                conversation_id: conversation_id.to_string(),
                through_seq,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.id)
    }

    pub async fn rename_conversation(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        title: &str,
    ) -> Result<(), ClientError> {
        self.conversations()
            .await?
            .update(UpdateConversationRequest {
                workspace_id: workspace_id.to_string(),
                conversation_id: conversation_id.to_string(),
                title: Some(title.to_string()),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    pub async fn delete_conversation(
        &self,
        workspace_id: &str,
        conversation_id: &str,
    ) -> Result<(), ClientError> {
        self.conversations()
            .await?
            .delete(DeleteConversationRequest {
                workspace_id: workspace_id.to_string(),
                conversation_id: conversation_id.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    /// Cancel an in-flight run (explicit user action — distinct from dropping a
    /// stream, which only detaches).
    pub async fn cancel_conversation(
        &self,
        workspace_id: &str,
        conversation_id: &str,
    ) -> Result<(), ClientError> {
        self.conversations()
            .await?
            .cancel(CancelConversationRequest {
                workspace_id: workspace_id.to_string(),
                conversation_id: conversation_id.to_string(),
            })
            .await
            .map_err(rpc)?;
        Ok(())
    }

    /// Queue a message behind the in-flight run; returns the resulting queue.
    pub async fn enqueue_message(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        message: &str,
    ) -> Result<Vec<QueuedMessage>, ClientError> {
        let resp = self
            .conversations()
            .await?
            .enqueue(EnqueueMessageRequest {
                workspace_id: workspace_id.to_string(),
                conversation_id: conversation_id.to_string(),
                message: message.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.queued.into_iter().map(QueuedMessage::from).collect())
    }

    /// Drop a queued message; returns the resulting queue.
    pub async fn dequeue_message(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        queued_id: &str,
    ) -> Result<Vec<QueuedMessage>, ClientError> {
        let resp = self
            .conversations()
            .await?
            .dequeue(DequeueMessageRequest {
                workspace_id: workspace_id.to_string(),
                conversation_id: conversation_id.to_string(),
                queued_id: queued_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(resp.queued.into_iter().map(QueuedMessage::from).collect())
    }

    /// Send a message and stream the run's [`ConvEvent`]s. Dropping the stream
    /// detaches without cancelling the run; a later [`Self::attach_conversation`]
    /// reattaches and resumes.
    pub async fn send_message(
        &self,
        workspace_id: &str,
        conversation_id: &str,
        message: &str,
        model: Option<String>,
    ) -> Result<impl Stream<Item = Result<ConvEvent, ClientError>>, ClientError> {
        let stream = self
            .conversations()
            .await?
            .send(SendMessageRequest {
                workspace_id: workspace_id.to_string(),
                conversation_id: conversation_id.to_string(),
                message: message.to_string(),
                model,
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(conv_event_stream(stream))
    }

    /// Attach to an in-flight run: replays buffered [`ConvEvent`]s then streams
    /// live ones. Dropping the stream detaches; the run continues.
    pub async fn attach_conversation(
        &self,
        workspace_id: &str,
        conversation_id: &str,
    ) -> Result<impl Stream<Item = Result<ConvEvent, ClientError>>, ClientError> {
        let stream = self
            .conversations()
            .await?
            .attach(AttachConversationRequest {
                workspace_id: workspace_id.to_string(),
                conversation_id: conversation_id.to_string(),
            })
            .await
            .map_err(rpc)?
            .into_inner();
        Ok(conv_event_stream(stream))
    }
}
