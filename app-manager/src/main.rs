use log::{debug, info};

use crate::{config::Config, manager::AppManager};

mod app;
mod config;
mod diskmanager;
mod dm;
mod dmcrypt;
mod dmverity;
mod keys;
mod manager;
mod utils;

static CONFIG: &'static str = r"
workdir: /workdir
vsock_port: 1337
crypto:
  cipher: Aes
  iv_mode: Plain
  block_mode: Cbc
  iv_offset: 0
";

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config: Config = serde_yaml::from_str(CONFIG)?;
    debug!("Using config: {:#?}", config);

    info!("Starting app-manager");

    let mut manager = AppManager::setup(config)?;

    info!("Loading realm info from host");
    manager.read_provision_info()?;

    info!("Decrypting applications main storage");
    manager.decrypt_main_storage()?;

    info!("Provisioning...");
    manager.provision()?;

    Ok(())
}
