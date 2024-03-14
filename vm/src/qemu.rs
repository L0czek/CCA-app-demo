use std::{env, process::{Child, Command}};

use thiserror::Error;

const QEMU_BIN: &'static str = "/usr/bin/qemu-system-aarch64";

#[derive(Error, Debug)]
pub enum QEMUError {
    #[error("Failed to start QEMU process")]
    FailedToStart(#[from] std::io::Error)
}

pub struct QEMURunner {
    command: Command
}

#[derive(Debug)]
pub struct QEMUInstance {
    process: Child
}

pub trait VMBuilder {
    fn cpu(&mut self, ty: &dyn AsRef<str>);
    fn machine(&mut self, ty: &dyn AsRef<str>);
    fn core_count(&mut self, n: usize);
    fn ram_size(&mut self, size_mb: usize);
    fn tap_device(&mut self, name: &dyn AsRef<str>);
    fn mac_addr(&mut self, addr: &dyn AsRef<str>);
    fn vsock_cid(&mut self, cid: usize);
    fn kernel(&mut self, image: &dyn AsRef<str>);
    fn block_device(&mut self, path: &dyn AsRef<str>);
    fn stdout(&mut self, path: &dyn AsRef<str>);
    fn arg(&mut self, arg: &dyn AsRef<str>);
}

impl QEMURunner {
    pub fn new() -> Self {
        let qemu = env::var("QEMU_BIN").unwrap_or(QEMU_BIN.to_string());

        Self {
            command: Command::new(qemu)
        }
    }

    pub fn launch(&mut self) -> Result<QEMUInstance, QEMUError> {
        println!("cmd: {:?}", self.command);
        Ok(QEMUInstance::new(
            self.command.spawn()
                .map_err(QEMUError::FailedToStart)?
        ))
    }
}

impl QEMUInstance {
    pub fn new(process: Child) -> Self {
        Self { process }
    }
}

impl VMBuilder for QEMURunner {
    fn cpu(&mut self, ty: &dyn AsRef<str>) {
        self.command.arg("-cpu").arg(ty.as_ref());
    }

    fn machine(&mut self, ty: &dyn AsRef<str>) {
        self.command.arg("-machine").arg(ty.as_ref());
    }

    fn core_count(&mut self, n: usize) {
        self.command.arg("-smp").arg(n.to_string());
    }

    fn ram_size(&mut self, size_mb: usize) {
        self.command.arg("-m").arg(size_mb.to_string());
    }

    fn tap_device(&mut self, name: &dyn AsRef<str>) {
        self.command.arg("-netdev").arg(format!("tap,id=mynet0,ifname={},script=no,downscript=no", name.as_ref()));
    }

    fn mac_addr(&mut self, addr: &dyn AsRef<str>) {
        self.command.arg("-device").arg(format!("e1000,netdev=mynet0,mac={}", addr.as_ref()));
    }

    fn vsock_cid(&mut self, cid: usize) {
        self.command.arg("-device").arg(format!("vhost-vsock-pci,id=vhost-vsock-pci0,guest-cid={}", cid));
    }

    fn kernel(&mut self, image: &dyn AsRef<str>) {
        self.command.arg("-kernel").arg(image.as_ref());
    }

    fn block_device(&mut self, path: &dyn AsRef<str>) {
        self.command.arg("-drive").arg(format!("file={}", path.as_ref()));
    }

    fn stdout(&mut self, path: &dyn AsRef<str>) {
        self.command.arg("-serial").arg(format!("file:{}", path.as_ref()));
    }

    fn arg(&mut self, arg: &dyn AsRef<str>) {
        self.command.arg(arg.as_ref());
    }
}
