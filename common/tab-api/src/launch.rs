//! Launches `tab-daemon` and `tab-pty` processes.
//! The initial launch occurs in the `tab-cli`, using the currently running executible id.
//! `tab` exposes a hidden `tab --_launch [daemon|pty]` argument, which is used here to launch associated services.
use crate::config::{is_running, load_daemon_file, DaemonConfig};
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
        let _child = Command::new(exec)
            .args(&["--_launch", "daemon"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(false)
            .spawn()?;
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
    let exec = std::env::current_exe()?;
    debug!("launching `tab-pty` at {}", &exec.to_string_lossy());

    let _child = Command::new(exec)
        .args(&["--_launch", "pty"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(false)
        .spawn()?;

    Ok(())
}

/// Waits for either a ctrl-c signal, or a message on the given channel.
///
/// Useful in main() functions.
pub async fn wait_for_shutdown<T>(mut receiver: impl Receiver<T>) {
    info!("Waiting for termination");

    loop {
        select! {
            _ = ctrl_c() => {
                break;
            },
            _ = receiver.recv() => {
                // wait just a few moments for messages to settle.
                // if we terminate immediately, there could be terminal I/O going on.
                // example:
                //   05:39:38 [ERROR] ERR: TerminalEchoService/stdout: task was cancelled
                time::delay_for(Duration::from_millis(20)).await;
                break;
            }
        }
    }

    info!("Complete.  Shutting down");
}
