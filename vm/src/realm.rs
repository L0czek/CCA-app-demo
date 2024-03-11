use std::{collections::HashMap, fs::{create_dir, create_dir_all}, path::PathBuf};

use thiserror::Error;
use tokio::net::unix::pid_t;

use crate::app::{Application, ApplicationConfig, ApplicationError};

#[derive(Error, Debug)]
pub enum RealmError {
    #[error("Application id {0} already exists")]
    AppExists(String),

    #[error("Application id {0} doesn't exist")]
    AppDoesNotExist(String),

    #[error("Cannot create workdir")]
    WorkdirMkdirFail(#[source] std::io::Error),

    #[error("Error while modyfing application")]
    AppError(#[from] ApplicationError)
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
    apps: HashMap<String, Application>
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
            apps: HashMap::new()
        })
    }

    pub fn create_application(&mut self, id: String, config: ApplicationConfig) -> Result<(), RealmError> {
        if self.apps.contains_key(&id) {
            Err(RealmError::AppExists(id))
        } else {
            self.apps.insert(id.clone(), Application::new(
                    self.workdir.join(id),
                    config
                )?
            );
            Ok(())
        }
    }

    pub fn launch(&mut self) -> Result<pid_t, RealmError> {


        Ok(0.into())
    }
}
