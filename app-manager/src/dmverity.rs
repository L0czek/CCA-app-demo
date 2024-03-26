use thiserror::Error;

use crate::dm::{DeviceHandle, DeviceHandleWrapper};

#[derive(Error, Debug)]
pub enum VerityDeviceError {

}

pub struct VerityDevice(pub DeviceHandle);

impl VerityDevice {
}

impl DeviceHandleWrapper for VerityDevice {
    fn dm_handle(&self) -> &DeviceHandle {
        &self.0
    }
}
