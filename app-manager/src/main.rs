use log::{debug, info};

use crate::{config::Config, manager::AppManager};

mod app;
mod config;
mod diskmanager;
mod dmcrypt;
mod keys;
mod manager;

static CONFIG: &'static str = r"
workdir: /workdir
vsock_port: 1337
";

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let config: Config = serde_yaml::from_str(CONFIG)?;
    debug!("Using config: {:#?}", config);

    info!("Starting app-manager");

    let mut manager = AppManager::setup(config)?;

    info!("Loading realm info from host");
    manager.read_provision_info()?;

    Ok(())
}
