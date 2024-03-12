use std::{collections::BTreeMap, fs::File, io::{Cursor, Seek, Write, Read}, os::unix::fs::MetadataExt, path::PathBuf};

use gpt::{mbr::ProtectiveMBR, partition_types, GptConfig};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum QEMUDiskError {
    #[error("Disk file opening error")]
    DiskFileOpenError(#[source] std::io::Error),

    #[error("Cannot fetch disk file stats")]
    DiskFileStatsError(#[source] std::io::Error),

    #[error("Exsiting disk `{0}` file size mismatch, expected {1} got {2}")]
    ExistingDiskSizeMismatch(PathBuf, usize, usize),

    #[error("Error opening disk by gpt crate")]
    GptOpenError(#[source] std::io::Error),

    #[error("Cannot create disk file")]
    DiskFileCreationError(#[source] std::io::Error),

    #[error("Cannot create sparse disk file")]
    SparseDiskCreationError(#[source] std::io::Error),

    #[error("Failed to initialize the GPT header")]
    GPTDiskHeaderInitError(#[source] std::io::Error),

    #[error("No free space on freshly created disk?")]
    GPTNoFreeSectors(),

    #[error("Failed to create the main partition")]
    GPTFailedToCreateMainPartition(#[source] std::io::Error),

    #[error("Failed to save configuration to disk file")]
    GPTFailedToSaveConfToDisk(#[source] std::io::Error),

    #[error("Error no partitions in initilized disk")]
    GPTErrorNoPartitions(),
}

#[derive(Debug)]
pub struct QEMUDisk {
    path: PathBuf,
    disk_uuid: Uuid,
    part_uuid: Uuid
}

impl QEMUDisk {
    pub fn new(path: PathBuf, size_mb: usize) -> Result<Self, QEMUDiskError> {
        let size_b = size_mb * 1024 * 1024;

        if path.exists() {
            let file_size_b = File::open(&path).map_err(QEMUDiskError::DiskFileOpenError)?
                .metadata().map_err(QEMUDiskError::DiskFileStatsError)?
                .size();

            if file_size_b != size_b as u64 {
                return Err(QEMUDiskError::ExistingDiskSizeMismatch(path.clone(), size_b, file_size_b as usize));
            }

         } else {

             let mut file = File::create_new(&path)
                 .map_err(QEMUDiskError::DiskFileCreationError)?;
             file.seek(std::io::SeekFrom::Start(size_b as u64 - 1))
                 .map_err(QEMUDiskError::SparseDiskCreationError)?;
             file.write_all(&[0u8])
                 .map_err(QEMUDiskError::SparseDiskCreationError)?;
             file.seek(std::io::SeekFrom::Start(0u64))
                 .map_err(QEMUDiskError::SparseDiskCreationError)?;

             let mbr = ProtectiveMBR::with_lb_size((size_b / 512 - 1) as u32);
             mbr.overwrite_lba0(&mut file)
                 .map_err(QEMUDiskError::SparseDiskCreationError)?;

             let mut gpt = GptConfig::new()
                 .writable(true)
                 .initialized(false)
                 .logical_block_size(gpt::disk::LogicalBlockSize::Lb4096)
                 .create_from_device(Box::new(&mut file), None)
                 .map_err(QEMUDiskError::GptOpenError)?;

              gpt.update_partitions(BTreeMap::new())
                  .map_err(QEMUDiskError::GPTDiskHeaderInitError)?;

              let free_sectors = gpt.find_free_sectors();

              if free_sectors.is_empty() {
                  return Err(QEMUDiskError::GPTNoFreeSectors());
              }

              gpt.add_partition(
                  "disk",
                  free_sectors[0].1 * 4096,
                  partition_types::LINUX_FS,
                  0,
                  None
              ).map_err(QEMUDiskError::GPTFailedToCreateMainPartition)?;

              gpt.write().map_err(QEMUDiskError::GPTFailedToSaveConfToDisk)?;
              file.sync_all()
                 .map_err(QEMUDiskError::SparseDiskCreationError)?;
        }

        let gpt = GptConfig::new()
            .writable(false)
            .initialized(true)
            .logical_block_size(gpt::disk::LogicalBlockSize::Lb4096)
            .open(&path)
            .map_err(QEMUDiskError::DiskFileOpenError)?;

        let disk_uuid = gpt.guid().clone();
        let (_, partition) = gpt.partitions()
            .first_key_value()
            .ok_or(QEMUDiskError::GPTErrorNoPartitions())?;
        let part_uuid = partition.part_guid;

        Ok(Self {
            path,
            disk_uuid,
            part_uuid
        })
    }

    pub fn path(&self) -> &PathBuf { &self.path }
    pub fn part_uuid(&self) -> &Uuid { &self.part_uuid }
    pub fn disk_uuid(&self) -> &Uuid { &self.part_uuid }
}
