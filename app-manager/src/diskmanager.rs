use std::{collections::HashMap, fs::File, io::{BufRead, BufReader}, path::PathBuf, str::FromStr};
use gpt::GptConfig;
use log::{debug, info};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum DiskManagerError {
    #[error("Error opening or reading /proc/partitions")]
    ProcPartitions(#[source] std::io::Error),

    #[error("Invalid /proc/partition format in line `{0}`")]
    ProcPartitionsFormat(String)
}

pub struct DiskManager {
    partitions: HashMap<Uuid, PathBuf>
}

impl DiskManager {
    pub fn available() -> Result<Self, DiskManagerError> {
        let partitions = File::open("/proc/partitions")
            .map_err(DiskManagerError::ProcPartitions)?;

        let mut manager = Self {
            partitions: HashMap::new()
        };

        let buf = BufReader::new(partitions);
        let mut it = buf.lines().skip(2);

        while let Some(line) = it.next() {
            let l = line
                .map_err(DiskManagerError::ProcPartitions)?;

            let devname = l.split_whitespace().nth(3)
                .ok_or(DiskManagerError::ProcPartitionsFormat(l.clone()))?;

            debug!("Discovered {} block device", devname);

            let diskpath = PathBuf::from_str("/dev").unwrap().join(devname);
            let gpt = GptConfig::new()
                .writable(false)
                .initialized(true)
                .logical_block_size(gpt::disk::LogicalBlockSize::Lb512)
                .open(&diskpath);

            if let Ok(disk) = gpt {
                for (id, partition) in disk.partitions().iter() {
                    let mut path = diskpath.clone().into_os_string();
                    path.push(id.to_string());
                    let uuid = partition.part_guid;

                    info!("Adding new partition path `{:?}`, uuid: {}", path, uuid);
                    manager.partitions.insert(uuid, path.into());
                }
            }
        }

        Ok(manager)
    }

    pub fn partition_path_by_uuid(&self, uuid: &Uuid) -> Option<&PathBuf> {
        self.partitions.get(uuid)
    }
}
