use thiserror::Error;

mod installer;
mod manifests;

#[derive(Error, Debug)]
pub enum DockerError {
    #[error("Docker installer error")]
    InstallerError(#[from] installer::InstallerError)
}
