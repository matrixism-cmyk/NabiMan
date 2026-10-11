use super::{
    protocol::{self, Reply, Request, VERSION},
    server::peer_uid,
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
}
