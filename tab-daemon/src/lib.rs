use daemonfile::DaemonFile;

use crate::bus::DaemonBus;
use log::{info, LevelFilter};

use lifeline::{dyn_bus::DynBus, prelude::*};
use message::daemon::DaemonShutdown;
use service::daemon::DaemonService;
use simplelog::{CombinedLogger, TermLogger, TerminalMode, WriteLogger};
use std::time::Duration;
use tab_api::{
    config::{daemon_log, DaemonConfig},
    launch::wait_for_shutdown,
    log::get_level,
};
use tab_websocket::resource::listener::{WebsocketAuthToken, WebsocketListenerResource};
use tokio::net::UnixListener;

mod auth;
mod bus;
mod daemonfile;
mod message;
mod prelude;
mod service;
mod state;

pub fn daemon_main(tab_version: &'static str) -> anyhow::Result<()> {
    let mut runtime = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();

    let result = runtime.block_on(async { main_async(tab_version).await });

    runtime.shutdown_timeout(Duration::from_millis(25));

    result?;

    Ok(())
}

pub async fn new_bus(tab_version: &'static str) -> anyhow::Result<DaemonBus> {
    let socket = tab_api::config::new_daemon_socket()?;
    let listener = UnixListener::bind(socket.as_path())?;
    let websocket = WebsocketListenerResource(listener);

    let auth_token = auth::gen_token();
    let pid = std::process::id();
    let executable = std::env::current_exe()
        .ok()
        .map(|path| path.to_str().map(str::to_string))
        .flatten();

    let config = DaemonConfig {
        pid: pid as i32,
        executable,
        socket: Some(socket),
        tab_version: Some(tab_version.to_string()),
        auth_token: auth_token.clone(),
    };

    let bus = DaemonBus::default();
    bus.store_resource::<DaemonConfig>(config);
    bus.store_resource::<WebsocketAuthToken>(auth_token.into());
    bus.store_resource::<WebsocketListenerResource>(websocket);

    Ok(bus)
}

async fn main_async(tab_version: &'static str) -> anyhow::Result<()> {
    let log_file = daemon_log()?;

    let config = simplelog::ConfigBuilder::new()
        .set_time_format_str("%H:%M:%S%.3f DAE")
        .build();

    let level = get_level().unwrap_or(LevelFilter::Info);
    CombinedLogger::init(vec![
        TermLogger::new(level, config.clone(), TerminalMode::Stderr),
        WriteLogger::new(level, config, std::fs::File::create(log_file)?),
    ])
    .unwrap();

    let bus = new_bus(tab_version).await?;
    let config = bus.resource::<DaemonConfig>()?;

    let daemon_file = DaemonFile::new(&config)?;
    info!("Daemon started.");
    info!("Daemon pid: {}", config.pid);

    let _service = DaemonService::spawn(&bus)?;
    let shutdown = bus.rx::<DaemonShutdown>()?;

    wait_for_shutdown(shutdown).await;

    info!("Daemon shutdown.");
    drop(daemon_file);

    Ok(())
}
