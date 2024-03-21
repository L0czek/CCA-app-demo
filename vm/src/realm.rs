use std::{collections::HashMap, fs::create_dir, path::PathBuf, sync::Arc, time::Duration};

use thiserror::Error;
use tokio::{select, spawn, sync::{oneshot::error::RecvError, Mutex}, task::{JoinHandle, JoinSet}, time};
use tokio_vsock::VsockStream;
use log::error;

use crate::{app::{Application, ApplicationConfig, ApplicationError}, daemon::DaemonContext, qemu::{QEMUError, QEMUInstance, QEMURunner, VMBuilder}, vsock::{ConnectionDispatcher, ConnectionDispatcherError}};
use protocol::RealmInfo;

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

    #[error("Realm launching error")]
    RealmLaunchingError(#[from] QEMUError),

    #[error("Error receiving Vsock stream")]
    VsockStreamRecv(#[source] ConnectionDispatcherError),

    #[error("Error while reading tokio oneshot channel")]
    ChannelError(#[from] RecvError),

    #[error("Protocol serialization error")]
    ProtocolError(#[from] serde_json::Error),

    #[error("Realm didn't connect")]
    VsockTimeout()
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

#[derive(Debug)]
pub struct Realm {
    workdir: PathBuf,
    config: RealmConfig,
    apps: HashMap<String, Application>,
    instance: Option<QEMUInstance>,
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
            instance: None,
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
        if self.instance.is_some() {
            return Err(RealmError::RealmAlreadyRunning());
        }

        self.configure(runner)?;
        self.instance = Some(runner.launch()?);

        let cid = self.config.vsock_cid as u32;
        let realm_info = self.realm_info();

        taskset.spawn(async move {
            let stream = ctx.dispatcher
                .lock().await
                .request_stream(cid)
                .map_err(RealmError::VsockStreamRecv)?;

            let timeout = time::sleep(Duration::from_secs(90));

            select! {
                v = stream => {
                    Realm::handle_realm(realm_info, v?).await
                }

                _ = timeout => {
                    error!("Timeout, realm didn't connect to vsock");
                    Err(RealmError::VsockTimeout())
                }

                _ = ctx.cancel.cancelled() => {
                    return Ok(());
                }
            }

        });

        Ok(())
    }

    async fn handle_realm(info: RealmInfo, stream: VsockStream) -> Result<(), RealmError> {
        serde_json::to_writer(stream, &info)?;
        Ok(())
    }

    fn realm_info(&self) -> RealmInfo {
        RealmInfo {
            apps: self.apps.iter().map(|(id, app)| (id.clone(), app.application_info())).collect()
        }
    }
}

