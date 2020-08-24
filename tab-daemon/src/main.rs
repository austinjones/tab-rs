use daemonfile::DaemonFile;

use crate::bus::DaemonBus;
use log::{info, LevelFilter};

use lifeline::{dyn_bus::DynBus, Bus, Service};
use message::daemon::DaemonShutdown;
use service::daemon::DaemonService;
use simplelog::{CombinedLogger, TermLogger, TerminalMode, WriteLogger};
use std::time::Duration;
use tab_api::config::{daemon_log, DaemonConfig};
use tab_websocket::resource::listener::{WebsocketAuthToken, WebsocketListenerResource};
use tokio::{net::TcpListener, select, signal::ctrl_c};

mod auth;
mod bus;
mod daemonfile;
mod message;
mod prelude;
mod pty_process;
mod service;
mod state;

pub fn main() -> anyhow::Result<()> {
    let mut runtime = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();

    let result = runtime.block_on(async { main_async().await });

    runtime.shutdown_timeout(Duration::from_millis(25));

    result?;

    Ok(())
}

pub async fn new_bus() -> anyhow::Result<DaemonBus> {
    let server = TcpListener::bind("127.0.0.1:0").await?;
    let port = server.local_addr()?.port();
    let websocket = WebsocketListenerResource(server);

    let auth_token = auth::gen_token();
    let pid = std::process::id();
    let config = DaemonConfig {
        pid: pid as i32,
        port,
        auth_token: auth_token.clone(),
    };

    let bus = DaemonBus::default();
    bus.store_resource::<DaemonConfig>(config);
    bus.store_resource::<WebsocketAuthToken>(auth_token.into());
    bus.store_resource::<WebsocketListenerResource>(websocket);

    Ok(bus)
}

async fn main_async() -> anyhow::Result<()> {
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

    let bus = new_bus().await?;
    let config = bus.resource::<DaemonConfig>()?;

    let daemon_file = DaemonFile::new(&config)?;
    info!("Daemon started.");
    info!("Daemon pid: {}", config.pid);
    info!("Daemon port: {}", config.port);

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
