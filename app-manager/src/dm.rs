use std::fs::read_dir;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use devicemapper::DeviceInfo;
use devicemapper::DM;
use devicemapper::DmOptions;
use devicemapper::DmName;
use devicemapper::DevId;
use devicemapper::DmFlags;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum DeviceMapperError {
    #[error("Unable to open device mapper")]
    OpenError(#[source] devicemapper::DmError),

    #[error("`{0}` is not a valid device name acording to device mapper")]
    InvalidName(String, #[source] devicemapper::DmError),

    #[error("Cannot create virtual mapping device named: {0}")]
    CreateError(String, #[source] devicemapper::DmError),

    #[error("Table load error")]
    TableLoad(#[source] devicemapper::DmError),

    #[error("Resume error")]
    ResumeError(#[source] devicemapper::DmError),

    #[error("Suspend Error")]
    SuspendError(#[source] devicemapper::DmError),

    #[error("Block device enumeration error")]
    EnumerationError(#[source] std::io::Error),

    #[error("Device `{0}` doesn't show as a block device, resume?")]
    DeviceNotShownInSysFs(String),

    #[error("Failed to read name from sysfs")]
    SysFsNameReadError(#[source] std::io::Error)
}

pub trait DeviceHandleWrapper {
    fn dm_handle(&self) -> &DeviceHandle;

    fn resume(&self) -> Result<(), DeviceMapperError> { self.dm_handle().resume() }
    fn suspend(&self) -> Result<(), DeviceMapperError> { self.dm_handle().suspend() }
    fn path(&self) -> Result<PathBuf, DeviceMapperError> { self.dm_handle().path() }
}

pub struct DeviceHandle {
    dm: Arc<DM>,
    info: DeviceInfo
}

impl DeviceHandle {
    pub fn new(dm: Arc<DM>, info: DeviceInfo) -> Self {
        Self { dm, info }
    }

    pub fn resume(&self) -> Result<(), DeviceMapperError> {
        let id = DevId::Name(self.info.name().unwrap());

        let _ = self.dm.device_suspend(&id, DmOptions::default())
            .map_err(DeviceMapperError::ResumeError)?;

        Ok(())
    }

    pub fn suspend(&self) -> Result<(), DeviceMapperError> {
        let id = DevId::Name(self.info.name().unwrap());

        let _ = self.dm.device_suspend(&id, DmOptions::default().set_flags(DmFlags::DM_SUSPEND))
            .map_err(DeviceMapperError::SuspendError)?;

        Ok(())
    }

    pub fn table_load(&self, targets: &[(u64, u64, String, String)], options: Option<DmOptions>) -> Result<(), DeviceMapperError> {
        let id = DevId::Name(self.info.name().unwrap());

        let _ = self.dm.table_load(&id, targets, options.unwrap_or(DmOptions::default()))
            .map_err(DeviceMapperError::TableLoad)?;

        Ok(())
    }

    pub fn path(&self) -> Result<PathBuf, DeviceMapperError> {
        for dir in read_dir("/sys/class/block/").map_err(DeviceMapperError::EnumerationError)? {
            let dir = dir.map_err(DeviceMapperError::EnumerationError)?;
            if let Ok(mut file) = File::open(dir.path().join("dm/name")) {
                let mut name = String::new();
                let _ = file.read_to_string(&mut name)
                    .map_err(DeviceMapperError::SysFsNameReadError)?;

                if name.trim() == self.info.name().unwrap().to_string() {
                    return Ok(PathBuf::from_str("/dev").unwrap().join(dir.file_name()));
                }
            }
        }

        Err(DeviceMapperError::DeviceNotShownInSysFs(self.info.name().unwrap().to_string()))
    }
}

pub struct DeviceMapper {
    dm: Arc<DM>,
}

impl DeviceMapper {
    pub fn init() -> Result<Self, DeviceMapperError> {
        Ok(Self {
            dm: Arc::new(DM::new().map_err(DeviceMapperError::OpenError)?),
        })
    }

    pub fn create(&self, name: &String, options: Option<DmOptions>) -> Result<DeviceHandle, DeviceMapperError> {
        let name = DmName::new(&name)
            .map_err(|e| DeviceMapperError::InvalidName(name.clone(), e))?;

        let info = self.dm.device_create(name, None, options.unwrap_or(DmOptions::default()))
            .map_err(|e| DeviceMapperError::CreateError(name.to_string(), e))?;

        Ok(DeviceHandle::new(self.dm.clone(), info))
    }

}
