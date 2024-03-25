use std::{collections::HashMap, fs::create_dir, sync::Arc};

use thiserror::Error;
use vsock::{VsockAddr, VsockStream, VMADDR_CID_ANY, VMADDR_CID_HOST, VMADDR_CID_LOCAL};
use log::{debug, info};
use protocol::RealmInfo;

use crate::{app::{Application, ApplicationError}, config::Config, diskmanager::{DiskManager, DiskManagerError}, dmcrypt::{DmCrypt, DmCryptError, Key}, keys::{KeyManager, KeyManagerError}};

#[derive(Error, Debug)]
pub enum AppManagerError {
    #[error("Unable to connect to host to get provisioning info")]
    ConnectionFailed(#[source] std::io::Error),

    #[error("Protocol error")]
    ProtocolError(#[from] serde_json::Error),

    #[error("ApplicationError")]
    AppError(#[from] ApplicationError),

    #[error("Disk manager error")]
    DiskManager(#[from] DiskManagerError),

    #[error("DmCrypt Error")]
    DmCryptError(#[from] DmCryptError),

    #[error("KeyManager Error")]
    KeyManagerError(#[from] KeyManagerError),

    #[error("Workdir creation error")]
    WorkdirCreation(#[source] std::io::Error)
}

pub struct AppManagerCtx {
    pub disks: DiskManager,
    pub dmcrypt: DmCrypt,
    pub keymanager: KeyManager
}

pub struct AppManager<'a> {
    ctx: Arc<AppManagerCtx>,
    config: Config,
    stream: VsockStream,
    apps: HashMap<String, Application<'a>>
}

impl<'a> AppManager<'a> {
    pub fn setup(config: Config) -> Result<Self, AppManagerError> {
        if !config.workdir.exists() {
            create_dir(&config.workdir).map_err(AppManagerError::WorkdirCreation)?;
        }

        let mut stream = VsockStream::connect(
            &VsockAddr::new(VMADDR_CID_HOST, config.vsock_port)
        ).map_err(AppManagerError::ConnectionFailed)?;

        debug!("Listing available block devices");
        let disks = DiskManager::available()?;

        debug!("Setting up DmCrypt");
        let dmcrypt = DmCrypt::init()?;

        debug!("Setting up key manager");
        let keymanager = KeyManager::new()?;

        let manager = Self {
            ctx: Arc::new(AppManagerCtx { disks, dmcrypt, keymanager }),
            config,
            stream,
            apps: HashMap::new()
        };

        Ok(manager)
    }

    pub fn read_provision_info(&mut self) -> Result<(), AppManagerError> {
        let info: RealmInfo = serde_json::from_reader(&mut self.stream)
            .map_err(AppManagerError::ProtocolError)?;

        debug!("Received RealmInfo: {:#?}", info);

        for (name, info) in info.apps.iter() {
            let workdir = self.config.workdir.join(name);
            self.apps.insert(name.clone(), Application::new(self.ctx.clone(), workdir, info.clone())?);
            info!("Added application: {}", name);
        }

        Ok(())
    }

    pub fn mount_main_storage(&'a mut self) -> Result<(), AppManagerError> {
        let row_realm_sealing_key = self.ctx.keymanager.realm_sealing_key()?;

        let key = Key::Raw(row_realm_sealing_key.to_vec());
        for (name, app) in self.apps.iter_mut() {
            app.mount_main_storage(&self.config.crypto, &key)?;
        }
        Ok(())
   }
}
