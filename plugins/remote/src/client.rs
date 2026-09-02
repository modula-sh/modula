//! The stub's half of `ModulaClient`. Signatures mirror the real plugin so the
//! desktop compiles against either; every call reports the feature is absent.

use async_trait::async_trait;
use modula_client::{ClientError, ModulaClient};

use crate::types::{RemoteDevice, RemoteStatus};

/// The QR the desktop renders, plus its expiry as unix epoch seconds.
pub struct PairingCode {
    pub qr_payload: String,
    pub expires_at: i64,
}

/// Every `RemoteService` call, over the caller's existing engine channel.
#[async_trait]
pub trait RemoteClient {
    async fn remote_status(&self) -> Result<RemoteStatus, ClientError>;
    async fn enable_remote(&self) -> Result<RemoteStatus, ClientError>;
    async fn disable_remote(&self) -> Result<RemoteStatus, ClientError>;
    async fn set_remote_password(&self, password: &str) -> Result<RemoteStatus, ClientError>;
    async fn begin_remote_pairing(&self) -> Result<PairingCode, ClientError>;
    async fn list_remote_devices(&self) -> Result<Vec<RemoteDevice>, ClientError>;
    async fn revoke_remote_device(&self, id: &str) -> Result<RemoteStatus, ClientError>;
    async fn set_remote_device_scope(
        &self,
        id: &str,
        scope: &str,
    ) -> Result<RemoteStatus, ClientError>;
}

fn unavailable<T>() -> Result<T, ClientError> {
    Err(ClientError::Rpc(
        "Modula Remote is not available in this build".into(),
    ))
}

#[async_trait]
impl RemoteClient for ModulaClient {
    async fn remote_status(&self) -> Result<RemoteStatus, ClientError> {
        unavailable()
    }
    async fn enable_remote(&self) -> Result<RemoteStatus, ClientError> {
        unavailable()
    }
    async fn disable_remote(&self) -> Result<RemoteStatus, ClientError> {
        unavailable()
    }
    async fn set_remote_password(&self, _password: &str) -> Result<RemoteStatus, ClientError> {
        unavailable()
    }
    async fn begin_remote_pairing(&self) -> Result<PairingCode, ClientError> {
        unavailable()
    }
    async fn list_remote_devices(&self) -> Result<Vec<RemoteDevice>, ClientError> {
        unavailable()
    }
    async fn revoke_remote_device(&self, _id: &str) -> Result<RemoteStatus, ClientError> {
        unavailable()
    }
    async fn set_remote_device_scope(
        &self,
        _id: &str,
        _scope: &str,
    ) -> Result<RemoteStatus, ClientError> {
        unavailable()
    }
}
