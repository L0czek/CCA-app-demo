use std::{fs::create_dir, path::PathBuf};

use thiserror::Error;
use tokio::task::JoinError;
use uuid::Uuid;

use crate::{qdisk::{QEMUDisk, QEMUDiskError}, qemu::VMBuilder};
use protocol::{ApplicationInfo, ProvisionInfo};

#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("Cannot create workdir")]
    WorkdirMkdirFail(#[source] std::io::Error),

    #[error("QEMU disk creation error")]
    QEMUDisk(#[from] QEMUDiskError),

    #[error("QEMU disk sync task joining error")]
    JoinError(#[from] JoinError),

    #[error("Path decoding error {0}")]
    PathDecodingError(PathBuf)
}

#[derive(Debug)]
pub struct ApplicationConfig {
    pub main_storage_size_mb: usize,
    pub secure_storage_size_mb: usize,
    pub provision_from: Option<Uuid>
}

#[derive(Debug)]
pub struct Application {
    workdir: PathBuf,
    config: ApplicationConfig,
    main_storage: QEMUDisk,
    secure_storage: QEMUDisk
}

impl Application {
    pub async fn new(workdir: PathBuf, config: ApplicationConfig) -> Result<Self, ApplicationError> {
        if ! workdir.exists() {
            create_dir(&workdir)
                .map_err(ApplicationError::WorkdirMkdirFail)?;
        }

        let main_storage_path = workdir.join("main.raw");
        let main_storage = tokio::task::spawn_blocking(move || {
            QEMUDisk::new(main_storage_path, config.main_storage_size_mb)
        });

        let secure_storage_path = workdir.join("secure.raw");
        let secure_storage = tokio::task::spawn_blocking(move || {
            QEMUDisk::new(secure_storage_path, config.secure_storage_size_mb)
        });

        Ok(Self {
            workdir,
            config,
            main_storage: main_storage.await??,
            secure_storage: secure_storage.await??
        })
    }

    pub fn configure(&self, builder: &mut dyn VMBuilder) -> Result<(), ApplicationError> {
        let main_storage_path = self.main_storage.path();
        builder.block_device(
            &main_storage_path.to_str()
                .ok_or(ApplicationError::PathDecodingError(main_storage_path.clone()))?
        );

        let secure_storage_path = self.secure_storage.path();
        builder.block_device(
            &secure_storage_path.to_str()
                .ok_or(ApplicationError::PathDecodingError(secure_storage_path.clone()))?
        );

        Ok(())
    }

    pub fn application_info(&self) -> ApplicationInfo {
        ApplicationInfo {
            main_partition_uuid: self.main_storage.part_uuid().clone(),
            secure_partition_uuid: self.secure_storage.part_uuid().clone(),
            provision_info: self.config.provision_from.as_ref().map(|uuid| ProvisionInfo { uuid: *uuid })
        }
    }
}
