use std::{fs::create_dir, future::Future, io::Error, path::{Path, PathBuf}, sync::Arc};

use tokio::{net::UnixListener, select, spawn, sync::Mutex, task::{JoinHandle, JoinSet}};
use log::{debug, info};
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use tokio_vsock::{VsockAddr, VsockListener, VMADDR_CID_ANY, VMADDR_CID_HOST, VMADDR_CID_HYPERVISOR};

use crate::{interface::ClientHandler, realm::RealmError, vsock::{ConnectionDispatcher, ConnectionDispatcherError}};

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Failed to create unix socket for cli interface")]
    CliSocketCreationFail(#[source] std::io::Error),

    #[error("Failed to accept new client")]
    ClientAcceptFail(#[source] std::io::Error),

    #[error("Cannot create workdir")]
    WorkdirMkdirFail(#[source] std::io::Error),

    #[error("Vsock binding error")]
    VsockBindError(#[source] std::io::Error),

    #[error("Vsock accept connection error")]
    VsockAcceptError(#[source] std::io::Error),

    #[error("Vsock connection dispatcher error")]
    VsockConnectionDispatcher(#[from] ConnectionDispatcherError)
}

#[derive(Debug)]
pub struct DaemonContext {
    pub workdir: PathBuf,
    pub cancel: CancellationToken,
    pub dispatcher: Mutex<ConnectionDispatcher>
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
           ctx: Arc::new(DaemonContext {
               workdir,
               cancel: CancellationToken::new(),
               dispatcher: Mutex::new(ConnectionDispatcher::new())
           })
        })
    }

    pub fn start_unixsocket_thread(&self, path: PathBuf) -> JoinHandle<Result<(), DaemonError>> {
        let ctx = self.ctx.clone();

        spawn(async move {
            Daemon::listen_unixsocket(ctx, path).await
        })
    }

    async fn listen_unixsocket(ctx: Arc<DaemonContext>, path: PathBuf) -> Result<(), DaemonError> {
        let mut set = JoinSet::new();
        debug!("Creating cli socket");

        let listener = UnixListener::bind(path)
            .map_err(DaemonError::CliSocketCreationFail)?;

        info!("Ready for connections");

        loop {
            select! {
                v = listener.accept() => {
                    let (stream, addr) = v.map_err(DaemonError::ClientAcceptFail)?;
                    info!("Accepted connection from {:?}", addr);

                    let ctx = ctx.clone();
                    let _ = set.spawn(async move {
                        let _ = ClientHandler::run(stream, ctx).await;
                        addr
                    });
                }

                v = set.join_next(), if !set.is_empty() => {
                    info!("Client {:?} exited", v);
                }

                _ = ctx.cancel.cancelled() => {
                    debug!("Unixsocket thread extiting");
                    break;
                }
            }
        }

        while let Some(v) = set.join_next().await {
            debug!("Client thread {:?} joined", v);
        }

        Ok(())
    }

    pub fn start_vsock_thread(&self, port: u32) -> JoinHandle<Result<(), DaemonError>> {
        let ctx = self.ctx.clone();

        spawn(async move {
            Daemon::listen_vsock(ctx, port).await
        })
    }

    async fn listen_vsock(ctx: Arc<DaemonContext>, port: u32) -> Result<(), DaemonError> {
        debug!("Listening on vsock port {}", port);

        let mut listener = VsockListener::bind(
            VsockAddr::new(VMADDR_CID_HOST, port)
        ).map_err(DaemonError::VsockBindError)?;

        info!("Ready for realm connections");

        loop {
            select! {
                v = listener.accept() => {
                    let (stream, addr) = v
                        .map_err(DaemonError::VsockAcceptError)?;
                    info!("Accepted vsock from {:?}", addr);

                    ctx.dispatcher.lock().await.add_stream(addr.cid(), stream)?;
                },

                _ = ctx.cancel.cancelled() => {
                    debug!("Vsock thread exiting");
                    break Ok(());
                }
            }
        }
    }

    pub fn shutdown(&self) {
        self.ctx.cancel.cancel();
    }
}
