use super::{
    protocol::{self, Reply, Request, SystemReply, VERSION},
    server::peer_uid,
    system_ops::{is_system, reply_timeout},
};
use std::{path::PathBuf, time::Duration};

#[derive(Clone)]
pub struct Client {
    path: PathBuf,
    helper_uid: u32,
}
#[derive(Debug)]
pub enum ClientError {
    Unavailable,
    UnknownResult,
}
impl Client {
    pub fn new(path: PathBuf, helper_uid: u32) -> Self {
        Self { path, helper_uid }
    }
    pub fn from_env() -> Self {
        Self::new(
            std::env::var_os("NABIMAN_HELPER_SOCKET")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/run/nabiman-helper/control.sock")),
            0,
        )
    }
    pub fn request(&self, request: &Request) -> Result<Reply, ClientError> {
        let mut socket =
            super::connect::connect(&self.path).map_err(|_| ClientError::Unavailable)?;
        if peer_uid(&socket).ok() != Some(self.helper_uid) {
            return Err(ClientError::Unavailable);
        }
        socket
            .set_read_timeout(Some(Duration::from_secs(25)))
            .map_err(|_| ClientError::Unavailable)?;
        socket
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|_| ClientError::Unavailable)?;
        protocol::write_frame(&mut socket, request).map_err(|_| ClientError::UnknownResult)?;
        let reply: Reply =
            protocol::read_frame(&mut socket).map_err(|_| ClientError::UnknownResult)?;
        if reply.version == VERSION
            && reply.request_id.is_empty()
            && reply.status == protocol::Status::Rejected
            && reply.receipt.is_none()
        {
            return Err(ClientError::Unavailable);
        }
        if reply.version != VERSION || reply.request_id != request.request_id {
            return Err(ClientError::UnknownResult);
        }
        if let Some(receipt) = &reply.receipt {
            if receipt.actor_id != request.actor_id || !protocol::is_id(&receipt.operation_id) {
                return Err(ClientError::UnknownResult);
            }
            let exact = match &request.operation {
                protocol::Operation::Inspect { operation_id } => {
                    &receipt.operation_id == operation_id
                }
                protocol::Operation::Execute { unit, action } => {
                    reply.status == protocol::Status::Conflict
                        || (receipt.operation_id == request.request_id
                            && receipt.unit == *unit
                            && receipt.action == *action)
                }
                // System operations are answered with a SystemReply (request_system).
                _ => false,
            };
            if !exact {
                return Err(ClientError::UnknownResult);
            }
        } else if matches!(
            reply.status,
            protocol::Status::Completed | protocol::Status::Conflict
        ) {
            return Err(ClientError::UnknownResult);
        }
        Ok(reply)
    }
    /// A system operation (firewall, packages): one request, one `SystemReply`,
    /// waited for as long as the operation's deadline allows. Never retried.
    pub fn request_system(&self, request: &Request) -> Result<SystemReply, ClientError> {
        if !is_system(&request.operation) {
            return Err(ClientError::UnknownResult);
        }
        let mut socket =
            super::connect::connect(&self.path).map_err(|_| ClientError::Unavailable)?;
        if peer_uid(&socket).ok() != Some(self.helper_uid) {
            return Err(ClientError::Unavailable);
        }
        socket
            .set_read_timeout(Some(reply_timeout(&request.operation)))
            .map_err(|_| ClientError::Unavailable)?;
        socket
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|_| ClientError::Unavailable)?;
        protocol::write_frame(&mut socket, request).map_err(|_| ClientError::UnknownResult)?;
        let value: serde_json::Value =
            protocol::read_frame_within(&mut socket, protocol::MAX_REPLY).map_err(|_| ClientError::UnknownResult)?;
        // The helper refuses an unpinned peer before reading, in the service format.
        if let Ok(refused) = serde_json::from_value::<Reply>(value.clone()) {
            if refused.request_id.is_empty() && refused.status == protocol::Status::Rejected {
                return Err(ClientError::Unavailable);
            }
            return Err(ClientError::UnknownResult);
        }
        let reply: SystemReply = serde_json::from_value(value).map_err(|_| ClientError::UnknownResult)?;
        if reply.version != VERSION
            || reply.request_id != request.request_id
            || (reply.status == protocol::Status::Completed) != reply.outcome.is_some()
        {
            return Err(ClientError::UnknownResult);
        }
        Ok(reply)
    }
}
