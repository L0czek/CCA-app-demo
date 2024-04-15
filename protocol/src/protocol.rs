use std::collections::HashMap;

use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProvisionInfo {

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
    Ok
}
