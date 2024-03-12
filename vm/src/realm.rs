use std::{collections::HashMap, fs::create_dir, path::PathBuf};

use thiserror::Error;

use crate::{app::{Application, ApplicationConfig, ApplicationError}, qemu::{QEMUError, QEMUInstance, QEMURunner, VMBuilder}};

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
    RealmLaunchingError(#[from] QEMUError)
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
    instance: Option<QEMUInstance>
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
            instance: None
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

    pub fn launch(&mut self, runner: &mut QEMURunner) -> Result<(), RealmError> {
        if self.instance.is_some() {
            return Err(RealmError::RealmAlreadyRunning());
        }

        self.configure(runner)?;
        self.instance = Some(runner.launch()?);

        Ok(())
    }
}

