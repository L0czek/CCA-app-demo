#![feature(result_flattening)]
#![feature(async_fn_in_trait)]
#![feature(let_chains)]

mod common;
mod hasher;
mod docker;
mod util;

use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;
use std::process::ExitStatus;

use async_trait::async_trait;
pub use hasher::Hasher;
pub use common::HashType;
use thiserror::Error;
use tokio::io::AsyncRead;
pub use docker::installer::Installer;
pub use docker::installer::InstallerError;
use tokio::task::JoinHandle;

#[derive(Error, Debug)]
pub enum ImageError {
    #[error("Docker image error")]
    DockerError(#[from] docker::DockerError),

    #[error("Error opening file")]
    FileOpenError(#[source] std::io::Error),

    #[error("Error reading file")]
    FileReadError(#[source] std::io::Error),

    #[error("Serde error, while parsing: {0:?}")]
    SerdeError(String, #[source] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ImageError>;


#[async_trait]
pub trait InstallerTrait {
    async fn install(&self, rot: Box<[u8]>, image: Box<dyn AsyncRead + Unpin + Send>) -> Result<Box<dyn Launcher>>;
    async fn validate(&self) -> Result<Box<dyn Launcher>>;
}



#[async_trait]
pub trait Launcher {
    fn launch(&mut self, disk_path: &PathBuf) -> Result<JoinHandle<Result<()>>>;
    async fn stop(&mut self) -> Result<ExitStatus>;
    async fn kill(&mut self) -> Result<ExitStatus>;
    async fn wait(&mut self) -> Result<ExitStatus>;
}

