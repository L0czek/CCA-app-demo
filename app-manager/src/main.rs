use log::info;

use crate::manager::AppManager;

mod manager;
mod protocol;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    info!("Starting app-manager");

    let manager = AppManager::connect(1337)?;

    Ok(())
}
