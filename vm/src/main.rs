#![feature(async_closure)]
#![feature(absolute_path)]

use std::{env::current_dir, fs::{canonicalize, remove_file}, path::{absolute, PathBuf}};

use clap::Parser;
use daemon::Daemon;
use log::debug;

mod app;
mod interface;
mod daemon;
mod realm;
mod qemu;
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
    let workdir = absolute(args.workdir)?;
    debug!("Workdir: {:?}", workdir);
    let daemon = Daemon::init(workdir)?;

    daemon.listen_unixsocket(args.cli_socket).await?;

    Ok(())
}
