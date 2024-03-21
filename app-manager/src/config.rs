use std::path::PathBuf;
use serde::Deserialize;


#[derive(Deserialize, Debug)]
pub struct Config {
    pub workdir: PathBuf,
    pub vsock_port: u32
}
