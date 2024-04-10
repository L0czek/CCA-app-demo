#![feature(result_flattening)]

mod common;
mod hasher;
mod docker;
mod util;

use std::future::Future;
use std::path::Path;

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

pub trait InstallerTrait {
    fn target(path: &Path) -> Self;
    fn install(&self, image: impl AsyncRead + Unpin) -> impl Future<Output = Result<Box<dyn Launcher>>>;
    fn validate(&self) -> impl Future<Output = Result<Box<dyn Launcher>>>;
}

pub trait Launcher {
    fn launch(&mut self) -> Result<JoinHandle<Result<()>>>;
}

