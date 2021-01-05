//! Launches `tab-daemon` and `tab-pty` processes.
//! The initial launch occurs in the `tab-cli`, using the currently running executible id.
//! `tab` exposes a hidden `tab --_launch [daemon|pty]` argument, which is used here to launch associated services.

use crate::{
    config::{is_running, load_daemon_file, DaemonConfig},
    env::is_raw_mode,
    log::get_level_str,
};
use anyhow::{bail, Context};
use log::*;
use nix::{
    sys::wait::{waitpid, WaitStatus},
    unistd::{fork, setsid, ForkResult},
};
use postage::stream::Stream;
use std::{
    path::Path,
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::{process::Command, select, signal::ctrl_c, time};

/// Launches a new daemon process (if it is not already running), and waits until it is ready for websocket connections.
pub async fn launch_daemon() -> anyhow::Result<DaemonConfig> {
    let exec = std::env::current_exe()?;
    let daemon_file = load_daemon_file()?;

    let running = daemon_file
        .as_ref()
        .map(|config| is_running(config))
        .unwrap_or(false);

    let start_wait = Instant::now();
    if !running {
        debug!("launching `tab-daemon` at {}", &exec.to_string_lossy());

        if let Err(e) = fork_launch_daemon(exec.as_path()) {
            warn!("Failed to launch daemon as a detached process.  Falling back to child process.  Cause: {}", e);
            spawn_daemon(exec.as_path())
                .context("Failed to launch daemon during child process fallback")?;
        }
    }

    let timeout_duration = Duration::from_secs(2);

    let mut index = 0usize;
    let daemon_file = loop {
        if let Some(daemon_file) = load_daemon_file()? {
            if is_running(&daemon_file) {
                break daemon_file;
            }
        }

        time::sleep(Duration::from_millis(50)).await;
        if Instant::now().duration_since(start_wait) > timeout_duration {
            return Err(anyhow::Error::msg("timeout while waiting for tab daemon"));
        }

        if index == 1 {
            info!("waiting for daemon...");
        }

        index += 1;
    };

    Ok(daemon_file)
}

/// Launches a new PTY process, which will connect to the running daemon.
pub fn launch_pty() -> anyhow::Result<()> {
    let exec = std::env::current_exe().context("failed to get current executable")?;
    debug!("launching `tab-pty` at {}", &exec.to_string_lossy());

    let mut child = Command::new(exec);
    child
        .args(&[
            "--_launch",
            "pty",
            "--log",
            get_level_str().unwrap_or("info"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(false);

    crate::env::forward_env(&mut child);

    let _child = child.spawn().context("failed to spawn child process")?;

    Ok(())
}

/// Waits for either a ctrl-c signal, or a message on the given channel.
///
/// Useful in main() functions.
pub async fn wait_for_shutdown<T: Default>(mut receiver: impl Stream<Item = T> + Unpin) -> T {
    loop {
        select! {
            _ = ctrl_c() => {
                return T::default();
            },
            msg = receiver.recv() => {
                return msg.unwrap_or(T::default());
            }
        }
    }
}

fn fork_launch_daemon(exec: &Path) -> anyhow::Result<()> {
    debug!("Forking the process to launch the daemon.");
    match unsafe { fork()? } {
        ForkResult::Parent { child } => {
            let result = waitpid(Some(child), None)?;
            if let WaitStatus::Exited(_pid, code) = result {
                if code != 0 {
                    bail!("Forked process exited with code {}", code);
                }
            }
        }
        ForkResult::Child => {
            let result: anyhow::Result<()> = {
                setsid()?;
                spawn_daemon(exec)?;
                Ok(())
            };

            let exit_code = result.map(|_| 0i32).unwrap_or(1i32);
            std::process::exit(exit_code);
        }
    }

    Ok(())
}

fn spawn_daemon(exec: &Path) -> anyhow::Result<()> {
    // because this is invoked in the forked process, we cannot use tokio
    let mut child = std::process::Command::new(exec);

    child
        .args(&[
            "--_launch",
            "daemon",
            "--log",
            get_level_str().unwrap_or("info"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null());

    if is_raw_mode() {
        child.stderr(Stdio::null());
    } else {
        child.stderr(Stdio::inherit());
    }

    crate::env::forward_env_std(&mut child);

    let _child = child.spawn()?;
    Ok(())
}
