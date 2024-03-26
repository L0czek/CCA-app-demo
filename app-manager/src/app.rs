use std::{fs::create_dir, path::PathBuf, sync::Arc};

use log::{debug, info};
use protocol::ApplicationInfo;
use thiserror::Error;
use uuid::Uuid;

use crate::{diskmanager::{DiskManager, DiskManagerError}, dm::DeviceMapperError, dmcrypt::{CryptDevice, CryptoParams, DmCryptError, DmCryptTable, Key}, manager::AppManagerCtx, utils::{format_ext2, mount_ext2, UtilitiesError}};
use crate::dm::DeviceHandleWrapper;

#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("Cannot create workdir")]
    WorkdirCreation(#[source] std::io::Error),

    #[error("Partition of uuid: {0} not found")]
    PartitionNotFound(Uuid),

    #[error("DmCrypt Error")]
    DmCryptError(#[from] DmCryptError),

    #[error("DiskManager Error")]
    DiskManagerError(#[from] DiskManagerError),

    #[error("Device mapper error")]
    DeviceMapperError(#[from] DeviceMapperError),

    #[error("Main storage was not decrypted")]
    MainStorageNotDecrypted(),

    #[error("Utilities error")]
    UtilitiesError(#[from] UtilitiesError),

    #[error("Failed to create mountpoint {0:?}")]
    MkdirError(PathBuf, #[source] std::io::Error)
}

pub struct Application {
    ctx: Arc<AppManagerCtx>,
    workdir: PathBuf,
    info: ApplicationInfo,
    main_storage: Option<CryptDevice>
}

impl Application {
    pub fn new(ctx: Arc<AppManagerCtx>, workdir: PathBuf, info: ApplicationInfo) -> Result<Self, ApplicationError> {
        if !workdir.exists() {
            create_dir(&workdir).map_err(ApplicationError::WorkdirCreation)?;
        }

        Ok(Self {
            ctx,
            workdir,
            info,
            main_storage: None
        })
    }

    pub fn decrypt_main_storage(&mut self, params: &CryptoParams, key: &Key) -> Result<(), ApplicationError> {
        let partition = self.ctx.disks.partition_path_by_uuid(&self.info.main_partition_uuid)
            .ok_or(ApplicationError::PartitionNotFound(self.info.main_partition_uuid.clone()))?;
        let crypt_device_name = self.info.main_partition_uuid.to_string();

        info!("Creating dmcrypt device {}", crypt_device_name);
        let device = CryptDevice(self.ctx.devicemapper.create(&crypt_device_name, None)?);
        let table = DmCryptTable {
            start: 0,
            len: partition.sz()?,
            params,
            offset: 0
        };

        debug!("Loading table for device with: {:#?}", table);
        device.load(table, &partition.path(), key, None)?;

        info!("Starting crypt device {}", crypt_device_name);
        device.resume()?;

        self.main_storage = Some(device);

        Ok(())
    }

    pub fn provision(&self) -> Result<(), ApplicationError> {
        if self.main_storage.is_none() {
            return Err(ApplicationError::MainStorageNotDecrypted());
        }

        let device = self.main_storage.as_ref().unwrap();
        let path = device.path()?;

        if self.info.provision_info.is_some() {
            info!("Formatting main storage");
            format_ext2(&path, Some(&"Main storage".to_owned()))?;
        }

        let target = self.workdir.join("main");
        create_dir(&target).map_err(|e| ApplicationError::MkdirError(target.clone(), e))?;

        info!("Mounting main storage in {:?}", target);
        mount_ext2(&path, &target)?;

        Ok(())
    }
}
