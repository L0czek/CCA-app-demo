use std::{fs::File, io::{Seek, Write}, os::unix::fs::MetadataExt, path::PathBuf};

use gpt::{GptConfig, GptDisk};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum QEMUDiskError {
    #[error("Disk file opening error")]
    DiskFileOpenError(#[source] std::io::Error),

    #[error("Cannot fetch disk file stats")]
    DiskFileStatsError(#[source] std::io::Error),

    #[error("Exsiting disk file size mismatch, expected {0} got {1}")]
    ExistingDiskSizeMismatch(usize, usize),

    #[error("Error opening disk by gpt crate")]
    GptOpenError(#[source] std::io::Error),

    #[error("Cannot create disk file")]
    DiskFileCreationError(#[source] std::io::Error),

    #[error("Cannot create sparse disk file")]
    SparseDiskCreationError(#[source] std::io::Error)
}

pub struct QEMUDisk {
    path: PathBuf
}

impl QEMUDisk {
    pub fn new(path: PathBuf, size_mb: usize) -> Result<Self, QEMUDiskError> {
        if path.exists() {
            let file_size_b = File::open(&path).map_err(QEMUDiskError::DiskFileOpenError)?
                .metadata().map_err(QEMUDiskError::DiskFileStatsError)?
                .size();

            if file_size_b != size_mb as u64 * 1024 {
                return Err(QEMUDiskError::ExistingDiskSizeMismatch(size_mb * 1024, file_size_b as usize));
            }

         } else {
            let mut file = File::create_new(&path)
                .map_err(QEMUDiskError::DiskFileCreationError)?;
            file.seek(std::io::SeekFrom::Start(size_mb as u64 * 1024))
                .map_err(QEMUDiskError::SparseDiskCreationError)?;
            file.write_all(&[0u8])
                .map_err(QEMUDiskError::SparseDiskCreationError)?;
        }

        Ok(Self {
            path: path.clone(),
        })
    }

    pub fn ensure_partition_exists(&mut self) -> Result<(), QEMUDiskError> {
        let header = GptConfig::new()
            .writable(true)
            .open(&self.path)
            .map_err(QEMUDiskError::GptOpenError)?;
    }
}
