use std::path::PathBuf;
use serde::Deserialize;

use crate::dmcrypt::CryptoParams;


#[derive(Deserialize, Debug)]
pub struct Config {
    pub workdir: PathBuf,
    pub vsock_port: u32,
    pub crypto: CryptoParams,
    pub image_registry: String
}
