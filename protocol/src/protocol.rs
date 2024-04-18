use std::{collections::HashMap, os::unix::process::ExitStatusExt, process::ExitStatus};

use uuid::Uuid;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProvisionInfo {
    pub uuid: Uuid
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApplicationInfo {
    pub main_partition_uuid: Uuid,
    pub secure_partition_uuid: Uuid,

    pub provision_info: Option<ProvisionInfo>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RealmInfo {
    pub apps: HashMap<String, ApplicationInfo>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Command {
    StartApp(String),
    TerminateApp(String),
    KillApp(String),
    Shutdown()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Response {
    Ok,

    #[serde(serialize_with = "serialize_exit_status")]
    #[serde(deserialize_with = "deserialize_exit_status")]
    ExitStatus(ExitStatus)
}

fn serialize_exit_status<S: Serializer>(status: &ExitStatus, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_i32(status.clone().into_raw())
}

fn deserialize_exit_status<'de, D: Deserializer<'de>>(d: D) -> Result<ExitStatus, D::Error> {
    let code = i32::deserialize(d)?;
    Ok(ExitStatus::from_raw(code))
}
