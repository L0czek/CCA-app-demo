use thiserror::Error;
use vsock::{VsockAddr, VsockStream, VMADDR_CID_HOST};
use log::debug;
use crate::protocol::RealmInfo;

#[derive(Error, Debug)]
pub enum AppManagerError {
    #[error("Unable to connect to host to get provisioning info")]
    ConnectionFailed(#[source] std::io::Error),

    #[error("Protocol error")]
    ProtocolError(#[from] serde_json::Error)
}

pub struct AppManager {
}

impl AppManager {
    pub fn connect(port: u32) -> Result<(), AppManagerError> {
        let mut stream = VsockStream::connect(
            &VsockAddr::new(VMADDR_CID_HOST, port)
        ).map_err(AppManagerError::ConnectionFailed)?;

        let manager = Self {};

        let info: RealmInfo = serde_json::from_reader(&mut stream)?;
        debug!("Received realminfo: {:#?}", info);

        Ok(())
    }
}
