use daemonfile::DaemonFile;
use endpoint::handle_request;

use crate::bus::DaemonBus;
use log::{error, info, LevelFilter};
use runtime::DaemonRuntime;
use service::daemon::DaemonService;
use session::DaemonSession;
use simplelog::{CombinedLogger, TermLogger, TerminalMode, WriteLogger};
use std::{sync::Arc, time::Duration};
use tab_api::{
    config::{daemon_log, DaemonConfig},
    response::Response,
};
use tab_service::{dyn_bus::DynBus, Service};
use tab_websocket::{resource::listener::WebsocketListenerResource, server::spawn_server};
use tokio::{
    net::{TcpListener, TcpStream},
    task,
};

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

    let service = DaemonService::spawn(&bus)?;

    // TODO: intelligent shutdown behavior
    tokio::time::delay_for(Duration::from_millis(60000)).await;

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
