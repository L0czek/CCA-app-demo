use std::{collections::HashMap, fs::File, io::{BufRead, BufReader, Read}, path::{Path, PathBuf}, str::FromStr};
use gpt::GptConfig;
use log::{debug, info};
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum DiskManagerError {
    #[error("Error opening or reading /proc/partitions")]
    ProcPartitions(#[source] std::io::Error),

    #[error("Invalid /proc/partition format in line `{0}`")]
    ProcPartitionsFormat(String),

    #[error("Cannot read partition size from sysfs")]
    SizeReadError(#[source] std::io::Error),

    #[error("{0:?} is not a valid size")]
    InvalidSize(String)
}

pub struct Partition {
    name: String
}

impl Partition {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub fn path(&self) -> PathBuf {
        PathBuf::from_str("/dev").unwrap()
            .join(&self.name)
    }

    pub fn sz(&self) -> Result<u64, DiskManagerError> {
        let sysfs_path = Path::new("/sys/class/block/")
            .join(&self.name)
            .join("size");

        let mut file = File::open(sysfs_path)
            .map_err(DiskManagerError::SizeReadError)?;

        let mut sz = String::new();
        file.read_to_string(&mut sz)
            .map_err(DiskManagerError::SizeReadError)?;
        let sz = sz.trim();

        Ok(u64::from_str(sz).map_err(|_| DiskManagerError::InvalidSize(sz.to_owned()))?)
    }
}

pub struct DiskManager {
    partitions: HashMap<Uuid, Partition>
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
                    let name = format!("{}{}", devname, id);
                    let uuid = partition.part_guid.clone();
                    info!("Adding new partition {}, uuid: {}", name, uuid);
                    manager.partitions.insert(uuid, Partition::new(name));
                }
            }
        }

        Ok(manager)
    }

    pub fn partition_path_by_uuid(&self, uuid: &Uuid) -> Option<&Partition> {
        self.partitions.get(uuid)
    }
}
