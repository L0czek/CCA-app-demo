use std::{ffi::{c_void, CStr, CString, NulError, OsStr}, os::unix::ffi::OsStrExt, path::{Path, PathBuf}, process::Command};

use log::debug;
use nix::{errno::Errno, libc::{c_char, mount}};
use thiserror::Error;


#[derive(Error, Debug)]
pub enum UtilitiesError {
    #[error("Error running mkfs.ext2")]
    MkfsSpawnError(#[source] std::io::Error),

    #[error("Error waiting for mkfs completion")]
    MkfsCompletionError(#[source] std::io::Error),

    #[error("Mounting error")]
    MountError(#[source] Errno),

    #[error("CString conversion error in {0:?}")]
    CStringConvError(PathBuf, #[source] NulError)
}

pub fn format_ext2(devpath: &Path, label: Option<impl AsRef<str>>) -> Result<(), UtilitiesError> {
    let mut cmd = Command::new("/bin/mkfs.ext2");

    if let Some(label) = label.as_ref() {
        cmd.arg("-L").arg(label.as_ref());
    }
    cmd.arg(devpath);

    let mut pid = cmd.spawn().map_err(UtilitiesError::MkfsSpawnError)?;
    let res = pid.wait().map_err(UtilitiesError::MkfsCompletionError)?;
    debug!("mkfs.ext2 existed with code: {}", res);

    Ok(())
}

pub fn mount_ext2(devpath: &Path, target: &Path) -> Result<(), UtilitiesError> {
    let src = CString::new(devpath.as_os_str().as_bytes())
        .map_err(|e| UtilitiesError::CStringConvError(devpath.to_owned(), e))?;
    let dst = CString::new(target.as_os_str().as_bytes())
        .map_err(|e| UtilitiesError::CStringConvError(target.to_owned(), e))?;
    let fs = CString::new("ext2").unwrap();

    let ret = unsafe {
        mount(
            src.as_ptr() as *const c_char,
            dst.as_ptr() as *const c_char,
            fs.as_ptr() as *const c_char,
            0,
            0 as *const c_void
        )
    };

    if ret != 0 {
        Err(UtilitiesError::MountError(Errno::last()))
    } else {
        Ok(())
    }
}

pub fn mount_overlay(lower: &Path, upper: &Path, work: &Path, target: &Path) -> Result<(), UtilitiesError> {
    let fs = CString::new("overlay").unwrap();
    let dst = CString::new(target.as_os_str().as_bytes())
        .map_err(|e| UtilitiesError::CStringConvError(target.to_owned(), e))?;
    let options = format!("lowerdir={},upperdir={},workdir={}",
        lower.to_string_lossy(), upper.to_string_lossy(), work.to_string_lossy());
    let opt = CString::new(options).unwrap();

    let ret = unsafe {
        mount(
            fs.as_ptr() as *const c_char,
            dst.as_ptr() as *const c_char,
            fs.as_ptr() as *const c_char,
            0,
            opt.as_ptr() as *const c_void
        )
    };


    if ret != 0 {
        Err(UtilitiesError::MountError(Errno::last()))
    } else {
        Ok(())
    }
}
