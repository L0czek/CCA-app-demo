#![feature(async_closure)]
#![feature(absolute_path)]

use std::{env::current_dir, fs::{canonicalize, remove_file}, future::IntoFuture, path::{absolute, PathBuf}};

use clap::Parser;
use daemon::Daemon;
use log::{debug, info, error};
use tokio::{join, select, signal::unix::{signal, SignalKind}, try_join};

mod app;
mod interface;
mod daemon;
mod protocol;
mod realm;
mod qemu;
mod qdisk;
mod vsock;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to command socket
    #[clap(short, long)]
    cli_socket: PathBuf,

    /// Path to work dir
    #[clap(short, long, default_value = "./workdir")]
    workdir: PathBuf,

    /// Vsock port to listen on
    #[clap(short, long, default_value_t = 1337)]
    port: u32,
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

    let mut unixsocket = daemon.start_unixsocket_thread(args.cli_socket);
    let mut vsocksocket = daemon.start_vsock_thread(args.port);

    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;

    select! {
        _ = sigint.recv() => {
            info!("SIGINT received shutting down");
            daemon.shutdown();
        }

        _ = sigterm.recv() => {
            info!("SIGTERM recevied shuttding down");
            daemon.shutdown();
        }

        v = &mut unixsocket => {
            error!("Error while listening on unixsocket: {:?}", v);
            daemon.shutdown();
        }

        v = &mut vsocksocket => {
            error!("Error while listening on vsock: {:?}", v);
            daemon.shutdown();
        }
    }

    debug!("Waitining for threads to finish");

    if !vsocksocket.is_finished() {
        vsocksocket.await??;
    }

    if !unixsocket.is_finished() {
        unixsocket.await??;
    }

    debug!("Threads joined");

    Ok(())
}
