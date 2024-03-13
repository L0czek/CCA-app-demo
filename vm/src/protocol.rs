use std::collections::HashMap;

use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ApplicationInfo {
    pub main_partition_uuid: String,
    pub secure_partition_uuid: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RealmInfo {
    pub apps: HashMap<String, ApplicationInfo>
}
