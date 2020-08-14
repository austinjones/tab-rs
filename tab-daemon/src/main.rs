use async_tungstenite::accept_async;
use daemonfile::DaemonFile;
use log::{info, LevelFilter};
use simplelog::{CombinedLogger, TermLogger, TerminalMode, WriteLogger};
use std::time::Duration;
use tab_common::config::{daemon_log, DaemonConfig};
use tokio::stream::StreamExt;
use tokio::{
    net::{TcpListener, TcpStream},
    task,
};

mod daemonfile;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log_file = daemon_log()?;

    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Debug,
            simplelog::Config::default(),
            TerminalMode::Stderr,
        ),
        WriteLogger::new(
            LevelFilter::Info,
            simplelog::Config::default(),
            std::fs::File::create(log_file)?,
        ),
    ])
    .unwrap();

    let mut server = TcpListener::bind("127.0.0.1:0").await?;
    let port = server.local_addr()?.port();

    let pid = std::process::id();
    let config = DaemonConfig { pid, port };

    let daemon_file = DaemonFile::new(&config)?;
    info!("Daemon started.");
    info!("Daemon pid: {}", pid);
    info!("Daemon port: {}", port);

    task::spawn(async move {
        while let Ok((stream, _addr)) = server.accept().await {
            // TODO: only accept connections from loopback address
            task::spawn(accept_connection(stream));
        }
    });

    // TODO: intelligent shutdown behavior
    tokio::time::delay_for(Duration::from_millis(30000)).await;

    info!("tab daemon shutting down...");
    drop(daemon_file);

    Ok(())
}

async fn accept_connection(stream: TcpStream) -> anyhow::Result<()> {
    let addr = stream.peer_addr()?;
    let mut connection = async_tungstenite::tokio::accept_async(stream).await?;

    info!("connection opened from `{}`", addr);

    while let Some(msg) = connection.next().await {
        let msg = msg?;
    }

    Ok(())
}
