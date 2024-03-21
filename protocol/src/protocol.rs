use std::collections::HashMap;

use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApplicationInfo {
    pub main_partition_uuid: Uuid,
    pub secure_partition_uuid: Uuid
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RealmInfo {
    pub apps: HashMap<String, ApplicationInfo>
}
