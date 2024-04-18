use std::{ffi::{OsStr, OsString}, path::{Path, PathBuf}};

use async_trait::async_trait;
use hex::FromHexError;
use log::{debug, info, warn};
use thiserror::Error;
use tokio::{fs::{create_dir, remove_dir_all, File}, io::AsyncReadExt};
use tokio_tar::Archive;

use crate::{docker::manifests::{ContainerConfig, ImageManifest, Manifest}, util::{discard_rest, read_measured}, Hasher, ImageError, InstallerTrait, Result};

use super::launcher::Launcher;


#[derive(Error, Debug)]
pub enum InstallerError {
    #[error("Directory creation error")]
    DirCreationError(#[source] std::io::Error),

    #[error("Archive unpack error")]
    ArchiveError(#[source] std::io::Error),

    #[error("Invalid image format, file: {0}")]
    InvalidImageFileError(PathBuf, #[source] std::io::Error),

    #[error("Invalid path {0:?}, expected <hash>...")]
    InvalidPath(PathBuf),

    #[error("Filesystem layer hash is not a valid hex string: {0:?}")]
    InvalidHex(String, #[source] FromHexError),

    #[error("Image is corrupted, hash mismatch. file: {0:?}, expected: {1:X?} got: {2:X?}")]
    HashMismatch(PathBuf, Box<[u8]>, Box<[u8]>),

    #[error("Empty manifest")]
    EmptyManifest(),

    #[error("Number of fs layers mismatch number of hashes in container config")]
    HashNumberMismatch(),

    #[error("Error while cleaning up image files")]
    CleanupError(#[source] std::io::Error),

    #[error("No container config for arm64 arch found")]
    NoImageForArch()
}

impl From<InstallerError> for ImageError {
    fn from(value: InstallerError) -> Self {
        ImageError::DockerError(value.into())
    }
}

pub struct Installer {
    dst: PathBuf
}

impl Installer {
    async fn read_manifest(&self, imgdir: &Path, rot: Option<Box<[u8]>>) -> Result<(ImageManifest, ContainerConfig)> {
        let manifest_path = imgdir.join("manifest.json");
        let (manifest, manifest_hash) = read_measured(crate::HashType::Sha256, &manifest_path).await?;

        if let Some(rot) = rot && rot != manifest_hash {
            return Err(InstallerError::HashMismatch(manifest_path, rot, manifest_hash).into());
        }

        let manifests = serde_json::from_str::<Vec<ImageManifest>>(&manifest)
            .map_err(|e| ImageError::SerdeError(manifest, e))?;

        for manifest in manifests.into_iter() {
            let config = self.read_container_config(imgdir, &manifest).await?;

            if config.arch == "arm64" {
                return Ok((manifest, config))
            }
        }

        return Err(InstallerError::NoImageForArch().into());
    }

    async fn read_container_config(&self, imgdir: &Path, manifest: &ImageManifest) -> Result<ContainerConfig> {
        let config_path = imgdir.join(&manifest.config);
        let (config, config_measurement) = read_measured(crate::HashType::Sha256, &config_path).await?;
        let config_hash: Box<[u8]> = manifest.config.split_once('.')
            .ok_or(InstallerError::InvalidPath(config_path.clone()))
            .map(|hash| hex::decode(hash.0).map_err(|e| InstallerError::InvalidHex(hash.0.to_owned(), e)))
            .flatten()?.into();

        if config_measurement != config_hash {
            return Err(InstallerError::HashMismatch(config_path, config_hash.into(), config_measurement).into());
        }

        let config: ContainerConfig = serde_json::from_str(&config)
            .map_err(|e| ImageError::SerdeError(config, e))?;

        Ok(config)
    }

    pub fn target(path: PathBuf) -> Self {
        Self { dst: path }
    }
}

#[async_trait]
impl InstallerTrait for Installer {
    async fn install(&self, rot: Box<[u8]>, image: Box<dyn tokio::io::AsyncRead + Unpin + Send>) -> crate::Result<Box<dyn crate::Launcher>> {
        let imgdir = self.dst.join("img");

        info!("Decompressing docker image");
        let mut archive = Archive::new(image);
        create_dir(&imgdir).await.map_err(InstallerError::DirCreationError)?;
        archive.unpack(&imgdir).await.map_err(InstallerError::ArchiveError)?;

        info!("Reading image manifest");
        let (manifest, config) = self.read_manifest(&imgdir, Some(rot)).await?;

        info!("Decompressing filesystem layers");
        let fsdir = self.dst.join("rootfs");
        create_dir(&fsdir).await.map_err(InstallerError::DirCreationError)?;

        if manifest.layers.len() != config.rootfs.diff_ids.len() {
            return Err(InstallerError::HashNumberMismatch().into());
        }

        for (path, digest) in manifest.layers.iter().zip(config.rootfs.diff_ids.iter()) {
            debug!("Decompressing {:?}", path);

            let mut reader = Hasher::new(
                digest.ty,
                File::open(imgdir.join(path)).await.map_err(|e| InstallerError::InvalidImageFileError(path.clone(), e))?
            );

            let mut archive = Archive::new(&mut reader);
            archive.unpack(&fsdir).await.map_err(InstallerError::ArchiveError)?;
            discard_rest(&mut reader).await;
            let measurement = reader.finalize();

            if measurement != digest.val {
                return Err(InstallerError::HashMismatch(path.clone(), digest.val.clone(), measurement).into());
            }
        }

        info!("Installation finished");
        info!("Application ready at {:?}", fsdir);

        Ok(Box::new(Launcher::new(fsdir, config)) as Box<dyn crate::Launcher>)
    }

    async fn validate(&self) -> crate::Result<Box<dyn crate::Launcher>> {
        let imgdir = self.dst.join("img");

        info!("Reading image manifest");
        let (_, config) = self.read_manifest(&imgdir, None).await?;

        let fsdir = self.dst.join("rootfs");
        info!("Application ready at {:?}", fsdir);
        Ok(Box::new(Launcher::new(fsdir, config)) as Box<dyn crate::Launcher>)
    }
}
