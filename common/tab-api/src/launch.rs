//! Launches `tab-daemon` and `tab-pty` processes.
//! The initial launch occurs in the `tab-cli`, using the currently running executible id.
//! `tab` exposes a hidden `tab --_launch [daemon|pty]` argument, which is used here to launch associated services.

use crate::{
    config::{is_running, load_daemon_file, DaemonConfig},
    env::is_raw_mode,
    log::get_level_str,
};
use anyhow::Context;
use lifeline::prelude::*;
use log::*;
use std::{
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

        let mut child = Command::new(exec);

        child
            .args(&[
                "--_launch",
                "daemon",
                "--log",
                get_level_str().unwrap_or("info"),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .kill_on_drop(false);

        if is_raw_mode() {
            child.stderr(Stdio::null());
        } else {
            child.stderr(Stdio::inherit());
        }

        crate::env::forward_env(&mut child);

        let _child = child.spawn()?;
    }

    let timeout_duration = Duration::from_secs(2);

    let mut index = 0usize;
    let daemon_file = loop {
        if let Some(daemon_file) = load_daemon_file()? {
            if is_running(&daemon_file) {
                break daemon_file;
            }
        }

        time::delay_for(Duration::from_millis(50)).await;
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
pub async fn wait_for_shutdown<T: Default>(mut receiver: impl Receiver<T>) -> T {
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
