use std::path::{Path, PathBuf};

use handler::{Hasher, InstallerTrait};
use tokio::{fs::File, io::AsyncReadExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let installer = handler::Installer::target(&Path::new("./root"));
    let mut file = File::open("./e.tar").await?;
    // let mut launcher = installer.install(&mut file).await?;
    let mut launcher = installer.validate().await?;
    let handle = launcher.launch()?;
    handle.await??;

    Ok(())
}
