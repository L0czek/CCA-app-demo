use thiserror::Error;

pub mod installer;
pub mod launcher;
mod manifests;

#[derive(Error, Debug)]
pub enum DockerError {
    #[error("Docker installer error")]
    InstallerError(#[from] installer::InstallerError),

    #[error("Docker launcher error")]
    LauncherError(#[from] launcher::LauncherError)
}
