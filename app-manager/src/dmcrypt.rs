use std::{fmt::{Debug, Display}, path::PathBuf};

use devicemapper::{DevId, DeviceInfo, DmError, DmFlags, DmOptions, DM};
use serde::Deserialize;
use thiserror::Error;

use crate::dm::{DeviceHandle, DeviceHandleWrapper, DeviceMapperError};

#[derive(Error, Debug)]
pub enum DmCryptError {
    #[error("Cannot convert path `{0:?}` to string")]
    PathConversion(PathBuf),

    #[error("Device mapper error")]
    DeviceMapperError(#[from] DeviceMapperError)
}

#[derive(Deserialize, Debug)]
pub enum Cipher {
    Aes,
    Twofish,
    Serpent
}

impl Display for Cipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cipher::Aes => write!(f, "aes"),
            Cipher::Twofish => write!(f, "twofish"),
            Cipher::Serpent => write!(f, "serpent")
        }
    }
}

#[derive(Deserialize, Debug)]
pub enum HashAlgo {
    Sha256
}

impl Display for HashAlgo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HashAlgo::Sha256 => write!(f, "sha256")
        }
    }
}

#[derive(Deserialize, Debug)]
pub enum IvMode {
    Plain,
    Plain64,
    Essiv(HashAlgo)
}

impl Display for IvMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IvMode::Plain => write!(f, "plain"),
            IvMode::Plain64 => write!(f, "plain64"),
            IvMode::Essiv(h) => write!(f, "essiv:{}", h)
        }
    }
}

#[derive(Deserialize, Debug)]
pub enum BlockMode {
    Cbc,
    Xts
}

impl Display for BlockMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockMode::Cbc => write!(f, "cbc"),
            BlockMode::Xts => write!(f, "xts")
        }
    }
}

pub enum KeyType {
    Logon,
    User,
    Encrypted
}

impl Display for KeyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyType::User => write!(f, "user"),
            KeyType::Logon => write!(f, "logon"),
            KeyType::Encrypted => write!(f, "encrypted")
        }
    }
}

pub enum Key {
    Raw(Vec<u8>),
    Hex(String),
    Keyring {
        key_size: usize,
        key_type: KeyType,
        key_desc: String
    }
}

impl Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Key::Hex(h) => write!(f, "{}", h),
            Key::Raw(v) => write!(f, "{}", hex::encode(v)),
            Key::Keyring { key_size, key_type, key_desc } =>
                write!(f, ":{}:{}:{}", key_size, key_type, key_desc)
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct CryptoParams {
    pub cipher: Cipher,
    pub iv_mode: IvMode,
    pub block_mode: BlockMode,
    pub iv_offset: usize,
    pub additional_options: Option<Vec<String>>
}

#[derive(Debug)]
pub struct DmCryptTable<'a> {
    pub start: u64,
    pub len: u64,
    pub params: &'a CryptoParams,
    pub offset: u64
}

pub struct CryptDevice(pub DeviceHandle);

impl CryptDevice {
    pub fn load(&self, entry: DmCryptTable, devpath: &PathBuf, key: &Key, options: Option<DmOptions>) -> Result<(), DmCryptError> {
        let mut params = format!("{}-{}-{} {} {} {} {}",
            entry.params.cipher,
            entry.params.block_mode,
            entry.params.iv_mode,
            key,
            entry.params.iv_offset,
            devpath.to_str().ok_or(DmCryptError::PathConversion(devpath.clone()))?,
            entry.offset
        );

        if let Some(opts) = &entry.params.additional_options {
            params.push_str(format!("{} {}", opts.len(), opts.join(" ")).as_str());
        }

        let table = vec![(
            entry.start,
            entry.len,
            "crypt".into(),
            params
        )];

        let _ = self.0.table_load(&table, options)?;

        Ok(())
    }
}

impl DeviceHandleWrapper for CryptDevice {
    fn dm_handle(&self) -> &crate::dm::DeviceHandle {
        &self.0
    }
}
