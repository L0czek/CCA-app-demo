use std::path::{Path, PathBuf};

use thiserror::Error;
use tokio::fs::create_dir;
use tokio_tar::Archive;

use crate::{ImageError, InstallerTrait};


#[derive(Error, Debug)]
pub enum InstallerError {
    #[error("Directory creation error")]
    DirCreationError(#[source] std::io::Error),

    #[error("Archive unpack error")]
    ArchiveError(#[source] std::io::Error)
}

impl From<InstallerError> for ImageError {
    fn from(value: InstallerError) -> Self {
        ImageError::DockerError(value.into())
    }
}

pub type Result<V> = std::result::Result<V, InstallerError>;

pub struct Installer {
    dst: PathBuf
}

impl InstallerTrait for Installer {
    fn target(path: &Path) -> Self {
        Self { dst: PathBuf::from(path) }
    }

    fn install(&self, image: impl tokio::io::AsyncRead + Unpin) -> impl std::future::Future<Output =  crate::Result<()>> {
        async move {
            let mut archive = Archive::new(image);
            let dir = self.dst.join("img");
            create_dir(&dir).await.map_err(InstallerError::DirCreationError)?;
            archive.unpack(&dir).await.map_err(InstallerError::ArchiveError)?;



            Ok(())
        }
    }
}
