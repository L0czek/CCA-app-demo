use std::{path::{Path, PathBuf}, time::Duration};

use handler::{Hasher, InstallerTrait};
use log::info;
use tokio::{fs::File, io::AsyncReadExt, select, time};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let installer = handler::Installer::target(&Path::new("./root"));
    let mut file = File::open("./e.tar").await?;
    // let mut launcher = installer.install(&mut file).await?;
    let mut launcher = installer.validate().await?;
    let mut handle = launcher.launch()?;
    let delay = time::sleep(Duration::from_secs(10));
    tokio::pin!(delay);

    select! {
        h = &mut handle => {
            let _ = h??;
            info!("App thread handling exited");
        }

        _ = &mut delay => {
            info!("Timeout shutting down");
            let v = launcher.stop().await?;
            info!("Process exited with: {:?}", v);
            handle.await??;
        }
    }


    Ok(())
}
