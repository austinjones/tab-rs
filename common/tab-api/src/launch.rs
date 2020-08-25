use crate::config::{is_running, load_daemon_file, DaemonConfig};
use log::*;
use std::{
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::{process::Command, select, signal::ctrl_c, sync::mpsc, time};

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

pub async fn wait_for_shutdown<T>(mut receiver: mpsc::Receiver<T>) {
    info!("Waiting for termination");

    loop {
        select! {
            _ = ctrl_c() => {
                break;
            },
            _ = receiver.recv() => {
                break;
            }
        }
    }

    info!("Complete.  Shutting down");
}
