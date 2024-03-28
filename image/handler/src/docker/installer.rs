use std::path::{Path, PathBuf};

use thiserror::Error;


#[derive(Error, Debug)]
pub enum InstallerError {

}

pub type Result<V> = std::result::Result<V, InstallerError>;

pub struct Installer {
    dst: PathBuf
}

impl Installer {
    pub fn target(path: &Path) -> Self {
        Self {
            dst: PathBuf::from(path)
        }
    }


}
