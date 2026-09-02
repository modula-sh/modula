//! The remote tunnel's passthrough: call any engine method by gRPC path with an
//! already-encoded body. The one intentional exception to "`ModulaClient`
//! methods return `modula_types`" — this is transport, not a domain API, and
//! only a plugin that tunnels engine calls should use it.

use tokio_stream::Stream;
use tonic::client::Grpc;
use tonic::codegen::http::uri::PathAndQuery;
use tonic::Status;

use crate::codec::RawCodec;
use crate::error::ClientError;
use crate::ModulaClient;

/// Matches the remote transport's frame cap: a reply too big to frame onto the
/// wire is useless anyway.
const MAX_RAW_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

impl ModulaClient {
    pub async fn call_raw(&self, path: PathAndQuery, payload: Vec<u8>) -> Result<Vec<u8>, Status> {
        let mut grpc = self.raw_grpc().await?;
        Ok(grpc
            .unary(tonic::Request::new(payload), path, RawCodec)
            .await?
            .into_inner())
    }

    pub async fn stream_raw(
        &self,
        path: PathAndQuery,
        payload: Vec<u8>,
    ) -> Result<impl Stream<Item = Result<Vec<u8>, Status>>, Status> {
        let mut grpc = self.raw_grpc().await?;
        Ok(grpc
            .server_streaming(tonic::Request::new(payload), path, RawCodec)
            .await?
            .into_inner())
    }

    async fn raw_grpc(&self) -> Result<Grpc<tonic::transport::Channel>, Status> {
        let channel = self
            .channel()
            .await
            .map_err(|e: ClientError| Status::unavailable(e.to_string()))?;
        let mut grpc = Grpc::new(channel).max_decoding_message_size(MAX_RAW_MESSAGE_SIZE);
        grpc.ready()
            .await
            .map_err(|e| Status::unavailable(format!("engine channel is not ready: {e}")))?;
        Ok(grpc)
    }
}
