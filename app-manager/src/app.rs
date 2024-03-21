use std::{fs::create_dir, path::PathBuf};

use protocol::ApplicationInfo;
use thiserror::Error;

use crate::{diskmanager::DiskManager, dmcrypt::DmCrypt};

#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("Cannot create workdir")]
    WorkdirCreation(#[source] std::io::Error)
}

pub struct Application {
    workdir: PathBuf,
    info: ApplicationInfo
}

impl Application {
    pub fn new(workdir: PathBuf, info: ApplicationInfo) -> Result<Self, ApplicationError> {
        if !workdir.exists() {
            create_dir(&workdir).map_err(ApplicationError::WorkdirCreation)?;
        }

        Ok(Self {
            workdir,
            info
        })
    }

}
