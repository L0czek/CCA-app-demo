use std::{collections::HashMap, fmt::Display, path::PathBuf, sync::Arc};

use clap::{crate_name, Parser, Subcommand};
use log::{debug, info};
use thiserror::Error;
use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufStream}, net::UnixStream, select, task::JoinSet};
use uuid::Uuid;

use crate::{app::ApplicationConfig, daemon::DaemonContext, qemu::{QEMURunner, VMBuilder}, realm::{NetworkConfig, Realm, RealmConfig, RealmError}};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Subcommand
    #[command(subcommand)]
    command: Command
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Define a realm
    CreateRealm {
        /// Realm id
        #[clap(short, long)]
        id: String,

        /// CPU type
        #[clap(short, long, default_value = "cortex-a57")]
        cpu: String,

        /// Machine type
        #[clap(short, long, default_value = "virt")]
        machine: String,

        /// CPU core count for realm
        #[clap(short = 'n', long, default_value_t = 2)]
        core_count: usize,

        /// RAM size
        #[clap(short, long, default_value_t = 2048)]
        ram_size: usize,

        /// TAP device to enable TCP/IP networking
        #[clap(short, long, default_value = "tap100")]
        tap_device: String,

        /// MAC address for realm's network card
        #[clap(short = 'a', long, default_value = "52:55:00:d1:55:01")]
        mac_addr: String,

        /// VSOCK cid for realm
        #[clap(short, long)]
        vsock_cid: usize,

        /// Path to kernel image
        #[clap(short, long)]
        kernel: PathBuf
    },

    /// List all realms
    ListRealms {},

    /// Create application in a realm
    CreateApplication {
        /// Application id
        #[clap(short, long)]
        id: String,

        /// Realm id
        #[clap(short, long)]
        realm_id: String,

        /// Main storage size in MB
        #[clap(short, long, default_value_t = 1024)]
        main_storage_size_mb: usize,

        /// Secure storage size in MB
        #[clap(short, long, default_value_t = 1024)]
        secure_storage_size_mb: usize,

        /// Provision from
        #[clap(short, long)]
        provision_from: Option<Uuid>
    },

    /// Launch a configured realm
    LaunchRealm {
        /// Realm id to launch
        #[clap(short, long)]
        id: String
    },

    /// Start a stopped application
    StartApp {
        /// Application id
        #[clap(short, long)]
        id: String,

        /// Realm id
        #[clap(short, long)]
        realm_id: String,
    },

    /// Terminate a running application
    TerminateApp {
        /// Application id
        #[clap(short, long)]
        id: String,

        /// Realm id
        #[clap(short, long)]
        realm_id: String,
    },

    /// Kill a running application
    KillApp {
        /// Application id
        #[clap(short, long)]
        id: String,

        /// Realm id
        #[clap(short, long)]
        realm_id: String,
    },

    /// Shutdown realm
    Shutdown {
        /// Realm id
        #[clap(short, long)]
        id: String,
    }
}

#[derive(Debug)]
enum CommandResult {
    RealmCreated,
    ApplicationCreated,
    RealmLaunched,
    Msg(String),
    ApplicationStarted,
    ApplicationExited,
    RealmExited,
}

impl Display for CommandResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            CommandResult::RealmCreated => write!(f, "RealmCreated"),
            CommandResult::ApplicationCreated => write!(f, "ApplicationCreated"),
            CommandResult::RealmLaunched => write!(f, "RealmLaunched"),
            CommandResult::Msg(v) => write!(f, "{}", v),
            CommandResult::ApplicationExited => write!(f, "ApplicationExited"),
            CommandResult::ApplicationStarted => write!(f, "ApplicationStarted"),
            CommandResult::RealmExited => write!(f, "RealmExited")
        }
    }
}

#[derive(Error, Debug)]
pub enum ClientHandlerError {
    #[error("Failed to read line from client socket")]
    CliSocketReadError(#[source] std::io::Error),

    #[error("Failed to write response to client")]
    CliSocketWriteError(#[source] std::io::Error),

    #[error("Shell split error")]
    ShellSplitError(),

    #[error("{0}")]
    CommandLineParsingError(String),

    #[error("Realm of id {0} already exists")]
    RealmExists(String),

    #[error("Realm of id {0} doesn't exist")]
    RealmDoesNotExist(String),

    #[error("Errror occured while modyfing realm")]
    RealmError(#[from] RealmError)
}

#[derive(Debug)]
pub struct ClientHandler {
    context: Arc<DaemonContext>,
    realms: HashMap<String, Realm>,
    handler_threads: JoinSet<Result<(), RealmError>>,
    stream: BufStream<UnixStream>
}

impl ClientHandler {
    pub async fn run(mut stream: UnixStream, ctx: Arc<DaemonContext>) -> Result<(), ClientHandlerError> {
        let mut handler = Self {
            realms: HashMap::new(),
            context: ctx.clone(),
            handler_threads: JoinSet::new(),
            stream: BufStream::new(stream)
        };

        handler.print_prompt().await?;

        loop {

            let mut line = String::new();

            select! {
                v = handler.stream.read_line(&mut line) => {
                    v.map_err(ClientHandlerError::CliSocketReadError)?;
                    let cont = handler.handle_user_line(&line).await?;

                    if !cont {
                        break;
                    }
                }

                t = handler.handler_threads.join_next(), if !handler.handler_threads.is_empty() => {
                    info!("Realm provisioning threads has exited: {:?}", t);
                    continue;
                }

                _ = ctx.cancel.cancelled() => {
                    debug!("Client handler thread exiting");
                    break;
                }
            }
        }

        while let Some(_) = handler.handler_threads.join_next().await {
            debug!("Handler thread exited gracefully");
        }

        Ok(())
    }

    async fn print_prompt(&mut self) -> Result<(), ClientHandlerError> {
        self.stream.write_all("> ".as_bytes())
            .await
            .map_err(ClientHandlerError::CliSocketWriteError)?;
        self.stream.flush()
            .await
            .map_err(ClientHandlerError::CliSocketWriteError)?;

        Ok(())
    }

    async fn handle_user_line(&mut self, line: &String) -> Result<bool, ClientHandlerError> {
        let line = line.trim();

        if line.is_empty() {
            return Ok(false);
        }

        debug!("Command: {:?}", line);

        let msg = match self.handle_cli(line).await {
            Ok(result) => format!("{}\n", result),
            Err(ClientHandlerError::CommandLineParsingError(err)) => format!("{}\n", err),
            Err(error) => format!("{:?}\n", error)
        };

        debug!("Result: {}", msg);

        self.stream.write_all(msg.as_bytes())
            .await
            .map_err(ClientHandlerError::CliSocketWriteError)?;
        self.stream.flush()
            .await
            .map_err(ClientHandlerError::CliSocketWriteError)?;
        self.print_prompt().await?;

        Ok(true)
    }

    async fn handle_cli<S: AsRef<str>>(&mut self, line: S) -> Result<CommandResult, ClientHandlerError> {
        let argv = shlex::split(line.as_ref())
            .ok_or(ClientHandlerError::ShellSplitError())?;
        let args = Args::try_parse_from(argv.into_iter())
            .map_err(|e| ClientHandlerError::CommandLineParsingError(e.render().to_string()))?;
        self.handle_command(args.command).await
    }

    async fn handle_command(&mut self, command: Command) -> Result<CommandResult, ClientHandlerError> {
        match command {
            Command::CreateRealm { id, cpu, machine, core_count, ram_size, tap_device, mac_addr, vsock_cid, kernel }
                => self.handle_create_realm(id, RealmConfig {
                    cpu,
                    machine,
                    core_count,
                    ram_size,
                    network_config: NetworkConfig { tap_device, mac_addr },
                    vsock_cid,
                    kernel
                }),

            Command::ListRealms {  } => self.handle_list_realms(),

            Command::CreateApplication { id, realm_id, main_storage_size_mb, secure_storage_size_mb, provision_from }
                => self.handle_create_application(id, realm_id, ApplicationConfig { main_storage_size_mb, secure_storage_size_mb, provision_from }).await,

            Command::LaunchRealm { id } => self.handle_launch_realm(id),
            Command::StartApp { id, realm_id } => self.handle_start_app(id, realm_id).await,
            Command::TerminateApp { id, realm_id } => self.handle_terminate_app(id, realm_id).await,
            Command::KillApp { id, realm_id } => self.handle_kill_app(id, realm_id).await,
            Command::Shutdown { id } => self.handle_shutdown(id).await
        }
    }

    async fn handle_create_application(&mut self, id: String, realm_id: String, config: ApplicationConfig) -> Result<CommandResult, ClientHandlerError> {
        self.realms.get_mut(&realm_id)
            .ok_or(ClientHandlerError::RealmDoesNotExist(realm_id))?
            .create_application(id, config).await?;
        Ok(CommandResult::ApplicationCreated)
    }

    fn handle_list_realms(&self) -> Result<CommandResult, ClientHandlerError> {
        Ok(CommandResult::Msg(format!("Realms: {:#?}\n", self.realms)))
    }

    fn handle_create_realm(&mut self, id: String, config: RealmConfig) -> Result<CommandResult, ClientHandlerError> {
        if self.realms.contains_key(&id) {
            Err(ClientHandlerError::RealmExists(id))
        } else {
            self.realms.insert(id.clone(), Realm::new(
                    self.context.workdir.join(&id),
                    config
                )?
            );
            Ok(CommandResult::RealmCreated)
        }
    }

    fn handle_launch_realm(&mut self, id: String) -> Result<CommandResult, ClientHandlerError> {
        let realm = self.realms.get_mut(&id)
            .ok_or(ClientHandlerError::RealmDoesNotExist(id))?;

        let mut runner = QEMURunner::new();
        runner.arg(&"-nographic");
        realm.launch(&mut runner, self.context.clone(), &mut self.handler_threads)?;

        Ok(CommandResult::RealmLaunched)
    }

    pub async fn handle_start_app(&mut self, id: String, realm_id: String) -> Result<CommandResult, ClientHandlerError> {
        let realm = self.realms.get_mut(&realm_id)
            .ok_or(ClientHandlerError::RealmDoesNotExist(realm_id))?;
        realm.start_app(id).await?;

        Ok(CommandResult::ApplicationStarted)
    }

    pub async fn handle_terminate_app(&mut self, id: String, realm_id: String) -> Result<CommandResult, ClientHandlerError> {
        let realm = self.realms.get_mut(&realm_id)
            .ok_or(ClientHandlerError::RealmDoesNotExist(realm_id))?;
        realm.terminate_app(id).await?;
        Ok(CommandResult::ApplicationExited)
    }

    pub async fn handle_kill_app(&mut self, id: String, realm_id: String) -> Result<CommandResult, ClientHandlerError> {
        let realm = self.realms.get_mut(&realm_id)
            .ok_or(ClientHandlerError::RealmDoesNotExist(realm_id))?;
        realm.kill_app(id).await?;
        Ok(CommandResult::ApplicationExited)
    }

    pub async fn handle_shutdown(&mut self, realm_id: String) -> Result<CommandResult, ClientHandlerError> {
        let realm = self.realms.get_mut(&realm_id)
            .ok_or(ClientHandlerError::RealmDoesNotExist(realm_id))?;
        realm.shutdown().await?;
        Ok(CommandResult::RealmExited)
    }
}
