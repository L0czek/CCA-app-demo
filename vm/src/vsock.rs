use std::{collections::HashMap, future::Future};

use thiserror::Error;
use tokio::sync::oneshot::{self, Receiver};
use tokio_vsock::VsockStream;

#[derive(Error, Debug)]
pub enum ConnectionDispatcherError {
    #[error("Connection from {0} is already present")]
    ConnectionPresent(u32),

    #[error("Request from {0} is already present")]
    RequestPresent(u32),

    #[error("Failed to send stream to receiver to realm {0}")]
    SendError(u32)
}

#[derive(Debug)]
pub struct ConnectionDispatcher {
    available: HashMap<u32, VsockStream>,
    requests: HashMap<u32, oneshot::Sender<VsockStream>>
}

impl ConnectionDispatcher {
    pub fn new() -> Self {
        Self {
            available: HashMap::new(),
            requests: HashMap::new()
        }
    }

    pub fn add_stream(&mut self, cid: u32, stream: VsockStream) -> Result<(), ConnectionDispatcherError> {
        if self.available.contains_key(&cid) {
            return Err(ConnectionDispatcherError::ConnectionPresent(cid));
        }
        self.available.insert(cid, stream);
        self.resolve(cid)?;

        Ok(())
    }

    pub fn request_stream(&mut self, cid: u32) -> Result<Receiver<VsockStream>, ConnectionDispatcherError> {
        if self.requests.contains_key(&cid) {
            return Err(ConnectionDispatcherError::RequestPresent(cid));
        }

        let (tx, rx) = oneshot::channel();
        self.requests.insert(cid, tx);
        self.resolve(cid)?;

        Ok(rx)
    }

    fn resolve(&mut self, cid: u32) -> Result<(), ConnectionDispatcherError> {
        if self.available.contains_key(&cid) && self.requests.contains_key(&cid) {
            let stream = self.available.remove(&cid).unwrap();
            let tx = self.requests.remove(&cid).unwrap();

            tx.send(stream)
                .map_err(|_| ConnectionDispatcherError::SendError(cid))?;
        }

        Ok(())
    }
}

