#![feature(async_closure)]

use std::{fs::remove_file, path::PathBuf};

use clap::Parser;
use daemon::Daemon;

mod app;
mod interface;
mod daemon;
mod realm;
mod qdisk;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to command socket
    #[clap(short, long)]
    cli_socket: PathBuf,

    /// Path to work dir
    #[clap(short, long, default_value = "./workdir")]
    workdir: PathBuf

}


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args = Args::parse();

    if args.cli_socket.exists() {
        remove_file(&args.cli_socket)?;
    }

    let daemon = Daemon::init(args.workdir);

    daemon.listen_unixsocket(args.cli_socket).await?;

    Ok(())
}
