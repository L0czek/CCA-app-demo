use std::{collections::HashMap, sync::Arc};

use thiserror::Error;
use log::{debug, info};
use protocol::{Command, RealmInfo, Response};
use tokio::{fs::create_dir, task::spawn_blocking};
use tokio_vsock::{VsockAddr, VsockStream, VMADDR_CID_HOST};

use crate::{app::{Application, ApplicationError}, config::Config, diskmanager::{DiskManager, DiskManagerError}, dm::{DeviceMapper, DeviceMapperError}, dmcrypt::{DmCryptError, Key}, keys::{KeyManager, KeyManagerError}, utils::{serde_read, serde_write, UtilitiesError}};

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
    WorkdirCreation(#[source] std::io::Error),

    #[error("Device mapper error")]
    DeviceMapperError(#[from] DeviceMapperError),

    #[error("Utilities error")]
    UtilitiesError(#[from] UtilitiesError)
}

pub struct AppManagerCtx {
    pub disks: DiskManager,
    pub devicemapper: DeviceMapper,
    pub keymanager: KeyManager
}

pub struct AppManager {
    ctx: Arc<AppManagerCtx>,
    config: Config,
    stream: VsockStream,
    apps: HashMap<String, Application>
}

impl AppManager {
    pub async fn setup(config: Config) -> Result<Self, AppManagerError> {
        if !config.workdir.exists() {
            create_dir(&config.workdir).await.map_err(AppManagerError::WorkdirCreation)?;
        }

        let stream = VsockStream::connect(
            VsockAddr::new(VMADDR_CID_HOST, config.vsock_port)
        ).await.map_err(AppManagerError::ConnectionFailed)?;

        debug!("Listing available block devices");
        let disks = DiskManager::available()?;

        debug!("Setting up DmCrypt");
        let devicemapper = DeviceMapper::init()?;

        debug!("Setting up key manager");
        let keymanager = KeyManager::new()?;

        let manager = Self {
            ctx: Arc::new(AppManagerCtx { disks, devicemapper, keymanager }),
            config,
            stream,
            apps: HashMap::new()
        };

        Ok(manager)
    }

    pub async fn read_provision_info(&mut self) -> Result<(), AppManagerError> {
        let info: RealmInfo = serde_read(&mut self.stream).await?;

        debug!("Received RealmInfo: {:#?}", info);

        for (name, info) in info.apps.iter() {
            let workdir = self.config.workdir.join(name);
            self.apps.insert(name.clone(), Application::new(self.ctx.clone(), workdir, info.clone())?);
            info!("Added application: {}", name);
        }

        Ok(())
    }

    pub fn decrypt_main_storage(&mut self) -> Result<(), AppManagerError> {
        let row_realm_sealing_key = self.ctx.keymanager.realm_sealing_key()?;
        let key = Key::Raw(row_realm_sealing_key.to_vec());

        for (name, app) in self.apps.iter_mut() {
            info!("Decrypting main storage for {}", name);
            app.decrypt_main_storage(&self.config.crypto, &key)?;
        }

        Ok(())
    }

    pub fn provision_app_image(&self) -> Result<(), AppManagerError> {
        for (name, app) in self.apps.iter() {
            info!("Provisioning image for {}", name);
            app.provision_app_image()?;
        }

        Ok(())
    }

    pub fn decrypt_secure_storage(&mut self) -> Result<(), AppManagerError> {
        let row_realm_sealing_key = self.ctx.keymanager.realm_sealing_key()?;
        let key = Key::Raw(row_realm_sealing_key.to_vec());
        // TODO: add key sealing here later

        for (name, app) in self.apps.iter_mut() {
            info!("Decrypting secure storage {}", name);
            app.decrypt_secure_storage(&self.config.crypto, &key)?;
        }

        Ok(())
    }

    pub fn provision_secure_storage(&self) -> Result<(), AppManagerError> {
        for (name, app) in self.apps.iter() {
            info!("Provisioning secure memory for {}", name);
            app.provision_secure_memory()?;
        }

        Ok(())
    }

    pub fn mount_overlay(&self) -> Result<(), AppManagerError> {
        for (name, app) in self.apps.iter() {
            info!("Mounting overlay for {}", name);
            app.mount_overlay()?;
        }

        Ok(())
    }

    fn handle_command(&mut self, command: &Command) -> Result<Response, AppManagerError> {
        match command {
            Command::Shutdown() => {
                Ok(Response::Ok)
            },

            _ => Ok(Response::Ok)
        }
    }

    pub async fn event_loop(&mut self) -> Result<(), AppManagerError> {
        loop {
            let req: Command = serde_read(&mut self.stream).await?;
            debug!("Received command: {:?}", req);
            let resp = self.handle_command(&req)?;
            debug!("Genereted response: {:?}", resp);
            serde_write(&mut self.stream, resp).await?;

            if let Command::Shutdown() = req {
                info!("Received shutdown request exiting");
                break Ok(());
            }
        }
    }
}
