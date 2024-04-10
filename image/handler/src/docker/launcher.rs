use std::{env::set_current_dir, ffi::OsString, os::unix::fs::chroot, path::PathBuf, process::Stdio};

use nix::{unistd::{getgid, getuid, setuid, Gid, Group, Uid, User, setgid}};
use thiserror::Error;
use tokio::{io::{AsyncReadExt, BufReader, AsyncBufReadExt}, process::{Child, Command}, select, task};
use log::info;

use crate::{docker::manifests::UserConfig, ImageError};

use super::manifests::{ContainerConfig, Id};

#[derive(Error, Debug)]
pub enum LauncherError {
    #[error("Argv is empty")]
    EmptyArgv(),

    #[error("Failed to resolve id for user/group {0:?}")]
    UidGidError(String, #[source] nix::errno::Errno),

    #[error("Application spawning error")]
    SpawnError(#[source] std::io::Error),

    #[error("Error reading spawned process IO")]
    IOReadError(#[source] std::io::Error),

    #[error("Error while awaiting the spawned application")]
    WaitpidError(#[source] std::io::Error)
}

type Result<V> = std::result::Result<V, LauncherError>;

impl From<LauncherError> for ImageError {
    fn from(value: LauncherError) -> Self {
        ImageError::DockerError(value.into())
    }
}

pub struct Launcher {
    rootfs: PathBuf,
    conf: ContainerConfig
}

impl Launcher {
    pub fn new(rootfs: PathBuf, config: ContainerConfig) -> Launcher {
        Self { rootfs, conf: config }
    }

    fn env(&self) -> &Vec<String> {
        self.conf.config.env.as_ref()
    }

    fn argv(&self) -> Vec<String> {
        if let Some(entry) = self.conf.config.entrypoint.as_ref() {
            entry.iter().chain(self.conf.config.cmd.iter()).map(|i| i.clone()).collect()
        } else {
            self.conf.config.cmd.clone()
        }
    }

    fn resolve_uid(&self, id: &Id) -> Result<Uid> {
        match id {
            Id::Id(v) => Ok(Uid::from_raw(*v)),
            Id::Name(name) => {
                if let Some(user) = User::from_name(&name)
                    .map_err(|e| LauncherError::UidGidError(name.clone(), e))? {
                    Ok(user.uid)
                } else {
                    Ok(getuid())
                }
            }
        }
    }

    fn resolve_gid(&self, id: &Id) -> Result<Gid> {
        match id {
            Id::Id(v) => Ok(Gid::from_raw(*v)),
            Id::Name(name) => {
                if let Some(user) = Group::from_name(&name)
                    .map_err(|e| LauncherError::UidGidError(name.clone(), e))? {
                    Ok(user.gid)
                } else {
                    Ok(getgid())
                }
            }
        }
    }

    async fn handler(mut process: Child) -> Result<()> {
        let mut stdout = BufReader::new(process.stdout.take().unwrap());
        let mut stderr = BufReader::new(process.stderr.take().unwrap());

        let mut stdout_open = true;
        let mut stderr_open = true;

        let mut stdout_line = String::new();
        let mut stderr_line = String::new();

        loop {
            select! {
                v = stdout.read_line(&mut stdout_line), if stdout_open => {
                    if v.map_err(LauncherError::IOReadError)? == 0 {
                        stdout_open = false;
                        continue;
                    }

                    info!("stdout: {}", stdout_line);
                }

                v = stderr.read_line(&mut stderr_line), if stderr_open => {
                    if v.map_err(LauncherError::IOReadError)? == 0 {
                        stderr_open = false;
                        continue;
                    }

                    info!("stderr: {}", stderr_line);
                }

                v = process.wait() => {
                    let result = v.map_err(LauncherError::WaitpidError)?;
                    info!("Application exited with {:?}", result);
                    break;
                }
            }
        }

        Ok(())
    }
}

impl crate::Launcher for Launcher {
    fn launch(&mut self) -> crate::Result<tokio::task::JoinHandle<crate::Result<()>>> {
        let env = self.env();
        let argv = self.argv();

        if argv.is_empty() {
            return Err(LauncherError::EmptyArgv().into());
        }

        let mut cmd = Command::new(&argv[0]);

        cmd.env_clear();
        cmd.envs(env.iter().map(|line| line.split_once("=").unwrap_or((line, ""))));
        cmd.args(argv.iter().skip(1));

        let rootfs = self.rootfs.clone();
        let chdir = self.conf.config.pwd.clone();
        let (uid, gid) = match self.conf.config.user.as_ref() {
            None => (getuid(), getgid()),
            Some(UserConfig { uid, gid: None }) => {
                (self.resolve_uid(&uid)?, getgid())
            }
            Some(UserConfig { uid, gid: Some(gid) }) => {
                (self.resolve_uid(&uid)?, self.resolve_gid(&gid)?)
            }
        };

        unsafe {
            cmd.pre_exec(move || {
                chroot(&rootfs)?;
                set_current_dir("/")?;

                if let Some(dir) = chdir.as_ref() {
                    set_current_dir(dir)?;
                }

                setuid(uid)?;
                setgid(gid)?;

                Ok(())
            });
        }

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let process = cmd.spawn()
            .map_err(LauncherError::SpawnError)?;

        Ok(task::spawn(async move {
            Ok(Self::handler(process).await?)
        }))
    }
}
