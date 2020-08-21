use daemonfile::DaemonFile;

use crate::bus::DaemonBus;
use log::{info, LevelFilter};

use service::daemon::DaemonService;

use message::daemon::DaemonShutdown;
use simplelog::{CombinedLogger, TermLogger, TerminalMode, WriteLogger};
use std::time::Duration;
use tab_api::config::{daemon_log, DaemonConfig};
use tab_service::{dyn_bus::DynBus, Bus, Service};
use tab_websocket::resource::listener::WebsocketListenerResource;
use tokio::{net::TcpListener, select, signal::ctrl_c};

mod bus;
mod daemonfile;
mod endpoint;
mod message;
mod pty_process;
mod runtime;
mod service;
mod session;
mod state;

#[tokio::main(max_threads = 32)]
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

    let server = TcpListener::bind("127.0.0.1:0").await?;
    let port = server.local_addr()?.port();
    let websocket = WebsocketListenerResource(server);

    let pid = std::process::id();
    let config = DaemonConfig { pid, port };

    let daemon_file = DaemonFile::new(&config)?;
    info!("Daemon started.");
    info!("Daemon pid: {}", pid);
    info!("Daemon port: {}", port);

    let bus = DaemonBus::default();
    bus.store_resource(config);
    bus.store_resource(websocket);

    let _service = DaemonService::spawn(&bus)?;
    let shutdown = bus.rx::<DaemonShutdown>()?;

    info!("Waiting for termination");
    loop {
        select! {
            _ = ctrl_c() => {
                break;
            },
            _ = shutdown => {
                break;
            }
        }
    }

    info!("tab daemon shutting down...");
    drop(daemon_file);

    Ok(())
}

// async fn accept_connection(runtime: Arc<DaemonRuntime>, stream: TcpStream) -> anyhow::Result<()> {
//     let addr = stream.peer_addr()?;

//     info!("connection opened from `{}`", addr);

//     let mut session = DaemonSession::new(runtime);
//     let (mut rx_request, tx_response) = spawn_server(stream, Response::is_close).await?;

//     while let Some(msg) = rx_request.recv().await {
//         handle_request(msg, &mut session, tx_response.clone()).await?
//     }

//     info!("connection closed from `{}`", addr);

//     Ok(())
// }
