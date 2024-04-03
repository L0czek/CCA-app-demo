mod hasher;
mod docker;

use std::future::Future;
use std::path::Path;

pub use hasher::Hasher;
pub use hasher::HashType;
use thiserror::Error;
use tokio::io::AsyncRead;

#[derive(Error, Debug)]
pub enum ImageError {
    #[error("Docker image error")]
    DockerError(#[from] docker::DockerError)
}

pub type Result<T> = std::result::Result<T, ImageError>;

pub trait InstallerTrait {
    fn target(path: &Path) -> Self;
    fn install(&self, image: impl AsyncRead + Unpin) -> impl Future<Output =  Result<()>>;
}

pub trait Launcher {
    fn rootfs(path: Path) -> Self;
}

