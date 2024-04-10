use std::{ffi::{OsStr, OsString}, path::{Path, PathBuf}};

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
    CleanupError(#[source] std::io::Error)
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
    async fn read_manifest(&self, imgdir: &Path) -> Result<(ImageManifest, Box<[u8]>)> {
        let manifest_path = imgdir.join("manifest.json");
        let (manifest, manifest_hash) = read_measured(crate::HashType::Sha256, &manifest_path).await?;
        let manifests = serde_json::from_str::<Vec<ImageManifest>>(&manifest)
            .map_err(|e| ImageError::SerdeError(manifest, e))?;
        Ok((manifests.into_iter().next().ok_or(InstallerError::EmptyManifest())?, manifest_hash))
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
}

impl InstallerTrait for Installer {
    fn target(path: &Path) -> Self {
        Self { dst: PathBuf::from(path) }
    }

    fn install(&self, image: impl tokio::io::AsyncRead + Unpin) -> impl std::future::Future<Output = crate::Result<Box<dyn crate::Launcher>>> {
        async move {
            let imgdir = self.dst.join("img");

            info!("Decompressing docker image");
            let mut archive = Archive::new(image);
            create_dir(&imgdir).await.map_err(InstallerError::DirCreationError)?;
            archive.unpack(&imgdir).await.map_err(InstallerError::ArchiveError)?;

            info!("Reading image manifest");
            let (manifest, _) = self.read_manifest(&imgdir).await?;

            info!("Reading container config");
            let config = self.read_container_config(&imgdir, &manifest).await?;

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
    }

    fn validate(&self) -> impl std::future::Future<Output = crate::Result<Box<dyn crate::Launcher>>> {
        async move {
            let imgdir = self.dst.join("img");

            info!("Reading image manifest");
            let (manifest, _) = self.read_manifest(&imgdir).await?;

            info!("Reading container config");
            let config = self.read_container_config(&imgdir, &manifest).await?;

            let fsdir = self.dst.join("rootfs");
            info!("Application ready at {:?}", fsdir);
            Ok(Box::new(Launcher::new(fsdir, config)) as Box<dyn crate::Launcher>)
        }
    }
}
