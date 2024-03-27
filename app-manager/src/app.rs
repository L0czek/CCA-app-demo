use std::{fs::create_dir, path::PathBuf, sync::Arc};

use log::{debug, info};
use protocol::ApplicationInfo;
use thiserror::Error;
use uuid::Uuid;

use crate::{diskmanager::{DiskManager, DiskManagerError, Partition}, dm::DeviceMapperError, dmcrypt::{CryptDevice, CryptoParams, DmCryptError, DmCryptTable, Key}, manager::AppManagerCtx, utils::{format_ext2, mount_ext2, mount_overlay, UtilitiesError}};
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

    #[error("Secure storage was not decrypted")]
    SecureStorageNotDecrypted(),

    #[error("Utilities error")]
    UtilitiesError(#[from] UtilitiesError),

    #[error("Failed to create mountpoint {0:?}")]
    MkdirError(PathBuf, #[source] std::io::Error)
}

pub struct Application {
    ctx: Arc<AppManagerCtx>,
    workdir: PathBuf,
    info: ApplicationInfo,
    main_storage: Option<CryptDevice>,
    secure_storage: Option<CryptDevice>
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
            main_storage: None,
            secure_storage: None
        })
    }

    fn decrypt_partition(&mut self, uuid: Uuid, params: &CryptoParams, key: &Key) -> Result<CryptDevice, ApplicationError> {
        let partition = self.ctx.disks.partition_path_by_uuid(&uuid)
            .ok_or(ApplicationError::PartitionNotFound(uuid.clone()))?;
        let crypt_device_name = uuid.to_string();

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

        Ok(device)
    }

    pub fn decrypt_main_storage(&mut self, params: &CryptoParams, key: &Key) -> Result<(), ApplicationError> {
        info!("Decrypting main partition");
        self.main_storage = Some(self.decrypt_partition(self.info.main_partition_uuid, params, key)?);
        Ok(())
    }

    fn mount_storage(&self, device: &impl DeviceHandleWrapper, target: impl AsRef<str>, label: impl AsRef<str>) -> Result<(), ApplicationError> {
        let path = device.path()?;

        if self.info.provision_info.is_some() {
            info!("Formatting storage: {}", label.as_ref());
            format_ext2(&path, Some(label.as_ref()))?;
        }

        let target = self.workdir.join(target.as_ref());
        create_dir(&target).map_err(|e| ApplicationError::MkdirError(target.clone(), e))?;

        info!("Mounting {:?} storage in {:?}", path, target);
        mount_ext2(&path, &target)?;

        Ok(())
    }

    pub fn provision_app_image(&self) -> Result<(), ApplicationError> {
        if self.main_storage.is_none() {
            return Err(ApplicationError::MainStorageNotDecrypted());
        }

        self.mount_storage(
            self.main_storage.as_ref().unwrap(),
            "main",
            "Main storage"
        )?;

        Ok(())
    }

    pub fn decrypt_secure_storage(&mut self, params: &CryptoParams, key: &Key) -> Result<(), ApplicationError> {
        info!("Decrypting secure memory partition");
        self.secure_storage = Some(self.decrypt_partition(self.info.secure_partition_uuid, params, key)?);
        Ok(())
    }

    pub fn provision_secure_memory(&self) -> Result<(), ApplicationError> {
        if self.secure_storage.is_none() {
            return Err(ApplicationError::SecureStorageNotDecrypted());
        }

        self.mount_storage(
            self.secure_storage.as_ref().unwrap(),
            "secure",
            "Secure storage"
        )?;

        Ok(())
    }

    pub fn mount_overlay(&self) -> Result<(), ApplicationError> {
        let lower = self.workdir.join("main");
        let upper = self.workdir.join("secure/data");
        let work = self.workdir.join("secure/work");
        let target = self.workdir.join("root");

        for dir in [&lower, &upper, &work, &target].iter() {
            if !dir.exists() {
                create_dir(dir).map_err(|e| ApplicationError::MkdirError(PathBuf::from(dir), e))?;
            }
        }

        debug!("Mounting overlay lower={:?}, upper={:?}, work={:?}, target={:?}", lower, upper, work, target);
        mount_overlay(&lower, &upper, &work, &target)?;

        Ok(())
    }
}
