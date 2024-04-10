use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fmt::Display;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde::de::Error;
use crate::common::HashType;

#[derive(Serialize, Deserialize, Debug)]
pub enum MediaType {
    #[serde(rename = "application/vnd.docker.distribution.manifest.v1+json")]
    OldManifest,

    #[serde(rename = "application/vnd.docker.distribution.manifest.v2+json")]
    NewManifest,

    #[serde(rename = "application/vnd.docker.distribution.manifest.list.v2+json")]
    ManifestList,

    #[serde(rename = "application/vnd.docker.container.image.v1+json")]
    ContainerConfig,

    #[serde(rename = "application/vnd.docker.image.rootfs.diff.tar.gzip")]
    Layer,

    #[serde(rename = "application/vnd.docker.image.rootfs.foreign.diff.tar.gzip")]
    ForeignLayer,

    #[serde(rename = "application/vnd.docker.plugin.v1+json")]
    PluginConfig
}

#[derive(Debug)]
pub struct Digest {
    pub ty: HashType,
    pub val: Box<[u8]>
}

impl Serialize for Digest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        serializer.serialize_str(format!("{}:{}", &self.ty, hex::encode(&self.val)).as_str())
    }
}


impl<'de> Deserialize<'de> for Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de> {
        let v: &str = Deserialize::deserialize(deserializer)?;
        let (ty, val) = v.split_once(':')
            .ok_or(D::Error::invalid_value(serde::de::Unexpected::Str(v), &"Expected hash_type:hash_value_hex"))?;

        let ty = match ty {
            x if x == "sha256" => HashType::Sha256,
            x if x == "sha512" => HashType::Sha512,
            _ => {
                return Err(D::Error::invalid_value(serde::de::Unexpected::Str(ty), &"sha256 or sha512"));
            }
        };

        let val = hex::decode(val)
            .map_err(|_| D::Error::invalid_value(serde::de::Unexpected::Str(val), &"Expected hex string"))?;

        Ok(Self {
            ty,
            val: val.into()
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Object {
    #[serde(rename = "mediaType")]
    pub media_type: MediaType,

    pub digest: Digest,
    pub size: usize
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: usize,

    #[serde(rename = "mediaType")]
    pub media_type: MediaType,

    pub config: Object,
    pub layers: Vec<Object>
}

#[derive(Debug)]
pub enum Id {
    Name(String),
    Id(u32)
}

impl From<&str> for Id {
    fn from(value: &str) -> Self {
        if let Ok(id) = u32::from_str_radix(value, 10) {
            Self::Id(id)
        } else {
            Self::Name(value.to_owned())
        }
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Id::Name(v) => f.write_str(&v),
            Id::Id(v) => write!(f, "{}", v)
        }
    }
}

#[derive(Debug)]
pub struct UserConfig {
    pub uid: Id,
    pub gid: Option<Id>
}

impl<'de> Deserialize<'de> for UserConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de> {
        let v = String::deserialize(deserializer)?;

        if let Some((uid, gid)) = v.split_once(":") {
            Ok(Self {
                uid: uid.into(),
                gid: Some(gid.into())
            })
        } else {
            Ok(Self {
                uid: v.as_str().into(),
                gid: None
            })
        }
    }
}

impl Serialize for UserConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        if let Some(gid) = self.gid.as_ref() {
            serializer.serialize_str(format!("{}:{}", self.uid, gid).as_str())
        } else {
            serializer.serialize_str(format!("{}", self.uid).as_str())
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LaunchConfig {
    #[serde(rename = "Env")]
    pub env: Vec<String>,

    #[serde(rename = "Cmd", default)]
    pub cmd: Vec<String>,

    #[serde(rename = "Entrypoint")]
    pub entrypoint: Option<Vec<String>>,

    #[serde(rename = "User")]
    pub user: Option<UserConfig>,

    #[serde(rename = "ExposedPorts")]
    pub ports: Option<HashMap<String, HashMap<(), ()>>>,

    #[serde(rename = "WorkingDir")]
    pub pwd: Option<String>,

    #[serde(rename = "ArgsEscaped")]
    pub args_escaped: bool,

    #[serde(rename = "OnBuild")]
    pub on_build: Option<String>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HistoryItem {
    pub created: DateTime<Utc>,
    pub created_by: String,

    #[serde(default)]
    pub comment: String,

    #[serde(default)]
    pub empty_layer: bool
}


#[derive(Serialize, Deserialize, Debug)]
pub struct RootFs {
    #[serde(rename = "type")]
    pub ty: String,

    pub diff_ids: Vec<Digest>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ContainerConfig {
    #[serde(rename = "architecture")]
    pub arch: String,

    pub created: DateTime<Utc>,
    pub os: String,

    pub config: LaunchConfig,
    pub history: Vec<HistoryItem>,
    pub rootfs: RootFs
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ImageManifest {
    #[serde(rename = "Config")]
    pub config: String,

    #[serde(rename = "RepoTags")]
    pub tags: Vec<String>,

    #[serde(rename = "Layers")]
    pub layers: Vec<PathBuf>
}
