use std::{collections::HashMap, fs::create_dir, path::PathBuf, sync::Arc, time::Duration};

use thiserror::Error;
use tokio::{io::BufReader, process::Child, select, spawn, sync::{mpsc::{self, channel, Receiver, Sender}, oneshot::error::RecvError, Mutex}, task::{JoinHandle, JoinSet}, time};
use tokio_vsock::VsockStream;
use log::{debug, error, info, warn};
use tokio::io::AsyncBufReadExt;

use crate::{app::{Application, ApplicationConfig, ApplicationError}, daemon::DaemonContext, qemu::{QEMUError, QEMURunner, VMBuilder}, utils::{serde_write, UtilitiesError}, vsock::{ConnectionDispatcher, ConnectionDispatcherError}};
use protocol::{Command, RealmInfo};
use crate::utils::serde_read;

#[derive(Error, Debug)]
pub enum RealmError {
    #[error("Application id {0} already exists")]
    AppExists(String),

    #[error("Application id {0} doesn't exist")]
    AppDoesNotExist(String),

    #[error("Cannot create workdir")]
    WorkdirMkdirFail(#[source] std::io::Error),

    #[error("Error while modifing application")]
    AppError(#[from] ApplicationError),

    #[error("Path decoding error {0}")]
    PathDecodingError(PathBuf),

    #[error("Realm is already running")]
    RealmAlreadyRunning(),

    #[error("Realm is not running")]
    RealmIsNotRunning(),

    #[error("Realm launching error")]
    RealmLaunchingError(#[from] QEMUError),

    #[error("Error receiving Vsock stream")]
    VsockStreamRecv(#[source] ConnectionDispatcherError),

    #[error("Error while reading tokio oneshot channel")]
    ChannelError(#[from] RecvError),

    #[error("Protocol serialization error")]
    ProtocolError(#[from] serde_json::Error),

    #[error("Realm didn't connect")]
    VsockTimeout(),

    #[error("Utilities error")]
    UtilitiesError(#[from] UtilitiesError),

    #[error("Realm IO read error")]
    RealmIOReadError(#[source] std::io::Error),

    #[error("Waitpid error")]
    WaitpidError(#[source] std::io::Error),

    #[error("Failed to send request across threads???")]
    RequestChannelError(#[from] mpsc::error::SendError<Request>),

    #[error("Failed to send response across threads???")]
    ResponseChannelError(#[from] mpsc::error::SendError<Response>),

    #[error("Channel was closed")]
    ChannelClosed(),
}

#[derive(Debug)]
pub struct NetworkConfig {
    pub tap_device: String,
    pub mac_addr: String
}

#[derive(Debug)]
pub struct RealmConfig {
    pub cpu: String,
    pub machine: String,

    pub core_count: usize,
    pub ram_size: usize,

    pub network_config: NetworkConfig,
    pub vsock_cid: usize,

    pub kernel: PathBuf,
}

enum Request {
    StartApp(String),
    TerminateApp(String),
    KillApp(String),
    Shutdown()
}

enum Response {
    RealmNotConnected,
    Ok
}

#[derive(Debug)]
pub struct Realm {
    workdir: PathBuf,
    config: RealmConfig,
    apps: HashMap<String, Application>,
    txrx: Option<(Sender<Request>, Receiver<Response>)>
}

impl Realm {
    pub fn new(workdir: PathBuf, config: RealmConfig) -> Result<Self, RealmError> {
        if ! workdir.exists() {
            create_dir(&workdir)
                .map_err(RealmError::WorkdirMkdirFail)?;
        }

        Ok(Self {
            workdir,
            config,
            apps: HashMap::new(),
            txrx: None
        })
    }

    pub async fn create_application(&mut self, id: String, config: ApplicationConfig) -> Result<(), RealmError> {
        if self.apps.contains_key(&id) {
            Err(RealmError::AppExists(id))
        } else {
            self.apps.insert(id.clone(), Application::new(
                    self.workdir.join(id),
                    config
                ).await?
            );
            Ok(())
        }
    }

    fn configure(&self, builder: &mut dyn VMBuilder) -> Result<(), RealmError> {
        let log = self.workdir.join("console.log");
        builder.stdout(
            &log.to_str()
                .ok_or(RealmError::PathDecodingError(log.clone()))?
        );
        // builder.arg(&"-serial");
        // builder.arg(&"tcp:localhost:1337");


        builder.cpu(&self.config.cpu);
        builder.machine(&self.config.machine);
        builder.core_count(self.config.core_count);
        builder.ram_size(self.config.ram_size);
        builder.tap_device(&self.config.network_config.tap_device);
        builder.mac_addr(&self.config.network_config.mac_addr);
        builder.vsock_cid(self.config.vsock_cid);

        let kernel_path = &self.config.kernel;
        builder.kernel(
            &kernel_path.to_str()
                .ok_or(RealmError::PathDecodingError(kernel_path.clone()))?
        );

        for (_, app) in self.apps.iter() {
            app.configure(builder)?;
        }

        Ok(())
    }

    pub fn launch(&mut self, runner: &mut QEMURunner, ctx: Arc<DaemonContext>, taskset: &mut JoinSet<Result<(), RealmError>>) -> Result<(), RealmError> {
        if self.txrx.is_some() {
            return Err(RealmError::RealmAlreadyRunning());
        }

        self.configure(runner)?;
        let process = runner.launch()?;

        let cid = self.config.vsock_cid as u32;
        let realm_info = self.realm_info();

        let (tx1, rx1) = channel(1);
        let (tx2, rx2) = channel(1);

        self.txrx = Some((tx1, rx2));

        taskset.spawn(async move {
            Self::handle_realm(ctx.clone(), process, tx2, rx1, realm_info, cid).await
        });

        Ok(())
    }

    async fn handle_realm(ctx: Arc<DaemonContext>, mut process: Child, tx: Sender<Response>, mut rx: Receiver<Request>, info: RealmInfo, cid: u32) -> Result<(), RealmError> {
        let mut stream_request = ctx.dispatcher
            .lock().await
            .request_stream(cid)
            .map_err(RealmError::VsockStreamRecv)?;

        let timeout = time::sleep(Duration::from_secs(90));
        tokio::pin!(timeout);

        let mut stream = None;

        let mut stdout = BufReader::new(process.stdout.take().unwrap());
        let mut stderr = BufReader::new(process.stderr.take().unwrap());

        let mut stdout_open = true;
        let mut stderr_open = true;

        loop {
            let mut stdout_line = String::new();
            let mut stderr_line = String::new();

            select! {
                v = &mut stream_request, if stream.is_none() => {
                    let mut socket = v?;
                    serde_write(&mut socket, &info).await?;
                    stream = Some(socket);
                }

                _ = &mut timeout => {
                    if stream.is_none() {
                        warn!("Timeout watiting for realm to connect to vsock");
                        break;
                    }
                }

                v = process.wait() => {
                    let result = v.map_err(RealmError::WaitpidError)?;
                    info!("Realm exited with {:?}", result);
                    break;
                }

                v = stdout.read_line(&mut stdout_line), if stdout_open => {
                    if v.map_err(RealmError::RealmIOReadError)? == 0 {
                        stdout_open = false;
                        continue;
                    }

                    info!("stdout: {}", stdout_line);
                }

                v = stderr.read_line(&mut stderr_line), if stderr_open => {
                    if v.map_err(RealmError::RealmIOReadError)? == 0 {
                        stderr_open = false;
                        continue;
                    }

                    info!("stderr: {}", stderr_line);
                }

                req = rx.recv() => {
                    if let Some(cmd) = req {
                        let resp = match cmd {
                            Request::Shutdown() => {
                                if let Some(mut s) = stream.as_mut() {
                                    serde_write(&mut s, Command::Shutdown()).await?;
                                    let _ = serde_read::<protocol::Response>(&mut s).await?;

                                    Response::Ok
                                } else {
                                    Response::RealmNotConnected
                                }
                            },

                            _ => { Response::Ok }
                        };

                        tx.send(resp).await?;
                    }
                }
            }
        }

        let _ = process.wait().await.map_err(RealmError::WaitpidError)?;
        info!("Realm shutdown, exiting");

        Ok(())
    }


    fn realm_info(&self) -> RealmInfo {
        RealmInfo {
            apps: self.apps.iter().map(|(id, app)| (id.clone(), app.application_info())).collect()
        }
    }

    async fn send_request(&mut self, req: Request) -> Result<Response, RealmError> {
        if let Some((tx, rx)) = self.txrx.as_mut() {
            tx.send(req).await?;
            let resp = rx.recv().await.ok_or(RealmError::ChannelClosed())?;
            Ok(resp)
        } else {
            Err(RealmError::RealmIsNotRunning())
        }
    }

    pub async fn start_app(&mut self, id: String) -> Result<(), RealmError> {
        match self.send_request(Request::StartApp(id)).await? {
            Response::Ok => Ok(()),
            Response::RealmNotConnected => Err(RealmError::RealmIsNotRunning())
        }
    }

    pub async fn terminate_app(&mut self, id: String) -> Result<(), RealmError> {
        match self.send_request(Request::TerminateApp(id)).await? {
            Response::Ok => Ok(()),
            Response::RealmNotConnected => Err(RealmError::RealmIsNotRunning())
        }
    }

    pub async fn kill_app(&mut self, id: String) -> Result<(), RealmError> {
        match self.send_request(Request::KillApp(id)).await? {
            Response::Ok => Ok(()),
            Response::RealmNotConnected => Err(RealmError::RealmIsNotRunning())
        }
    }

    pub async fn shutdown(&mut self) -> Result<(), RealmError> {
        debug!("Sending shutdown request");
        match self.send_request(Request::Shutdown()).await? {
            Response::Ok => Ok(()),
            Response::RealmNotConnected => Err(RealmError::RealmIsNotRunning())
        }
    }
}

