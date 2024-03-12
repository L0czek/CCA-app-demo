use std::{fs::create_dir, io::Error, path::{Path, PathBuf}, sync::Arc};

use tokio::net::UnixListener;
use log::{debug, info};
use thiserror::Error;

use crate::{interface::ClientHandler, realm::RealmError};

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Failed to create unix socket for cli interface")]
    CliSocketCreationFail(#[source] std::io::Error),

    #[error("Failed to accept new client")]
    ClientAcceptFail(#[source] std::io::Error),

    #[error("Cannot create workdir")]
    WorkdirMkdirFail(#[source] std::io::Error)
}

#[derive(Debug)]
pub struct DaemonContext {
    pub workdir: PathBuf,
}

pub struct Daemon {
    ctx: Arc<DaemonContext>
}

impl Daemon {
    pub fn init(workdir: PathBuf) -> Result<Self, DaemonError> {
        if ! workdir.exists() {
            create_dir(&workdir)
                .map_err(DaemonError::WorkdirMkdirFail)?;
        }

        Ok(Self {
           ctx: Arc::new(DaemonContext { workdir })
        })
    }

    pub async fn listen_unixsocket<P: AsRef<Path>>(&self, path: P) -> Result<(), DaemonError> {
        debug!("Creating cli socket");

        let listener = UnixListener::bind(path.as_ref())
            .map_err(DaemonError::CliSocketCreationFail)?;

        info!("Ready for connections");

        loop {
            let (stream, addr) = listener.accept()
                .await
                .map_err(DaemonError::ClientAcceptFail)?;

            info!("Accepted connection from {:?}", addr);

            let ctx = Arc::clone(&self.ctx);
            tokio::spawn(async move {
                let _ = ClientHandler::run(stream, ctx).await;
            });
        }
    }
}
