use std::{fs::create_dir, path::PathBuf, sync::Arc};

use log::{debug, info};
use protocol::ApplicationInfo;
use thiserror::Error;
use uuid::Uuid;

use crate::{diskmanager::{DiskManager, DiskManagerError}, dmcrypt::{CryptDevice, CryptoParams, DmCrypt, DmCryptError, DmCryptTable, Key}, manager::AppManagerCtx};

#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("Cannot create workdir")]
    WorkdirCreation(#[source] std::io::Error),

    #[error("Partition of uuid: {0} not found")]
    PartitionNotFound(Uuid),

    #[error("DmCrypt Error")]
    DmCryptError(#[from] DmCryptError),

    #[error("DiskManager Error")]
    DiskManagerError(#[from] DiskManagerError)
}

pub struct Application<'a> {
    ctx: Arc<AppManagerCtx>,
    workdir: PathBuf,
    info: ApplicationInfo,
    main_storage: Option<CryptDevice<'a>>
}

impl<'a> Application<'a> {
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

    pub fn mount_main_storage(&'a mut self, params: &CryptoParams, key: &Key) -> Result<(), ApplicationError> {
        let partition = self.ctx.disks.partition_path_by_uuid(&self.info.main_partition_uuid)
            .ok_or(ApplicationError::PartitionNotFound(self.info.main_partition_uuid.clone()))?;
        let crypt_device_name = self.info.main_partition_uuid.to_string();

        info!("Creating dmcrypt device {}", crypt_device_name);
        let device = self.ctx.dmcrypt.create(&crypt_device_name, None)?;
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
}
