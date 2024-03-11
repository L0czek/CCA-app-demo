use std::{fs::create_dir, path::PathBuf};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("Cannot create workdir")]
    WorkdirMkdirFail(#[source] std::io::Error)
}

#[derive(Debug)]
pub struct ApplicationConfig {
    pub main_storage_size_mb: usize,
    pub secure_storage_size_mb: usize
}

#[derive(Debug)]
pub struct Application {
    workdir: PathBuf,
    config: ApplicationConfig
}

impl Application {
    pub fn new(workdir: PathBuf, config: ApplicationConfig) -> Result<Self, ApplicationError> {
        if ! workdir.exists() {
            create_dir(&workdir)
                .map_err(ApplicationError::WorkdirMkdirFail)?;
        }

        Ok(Self {
            workdir,
            config
        })
    }
}
