use crate::prelude::*;

use message::pty::MainShutdown;
use postage::{sink::Sink, stream::Stream};
use simplelog::{CombinedLogger, TermLogger, TerminalMode, WriteLogger};
use std::time::Duration;
use tab_api::{config::pty_log, launch::*, log::get_level, pty::PtyWebsocketRequest};

use lifeline::dyn_bus::DynBus;
use service::main::MainService;
use tab_websocket::resource::connection::WebsocketResource;

mod bus;
mod message;
mod prelude;
mod service;

pub fn pty_main() -> anyhow::Result<()> {
    init()?;

    debug!("pty process started");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();

    let result = runtime.block_on(async { main_async().await });

    runtime.shutdown_timeout(Duration::from_millis(25));
    debug!("pty process terminated");

    result?;

    Ok(())
}

fn init() -> anyhow::Result<()> {
    let log_file = pty_log()?;

    let config = simplelog::ConfigBuilder::new()
        .set_time_format_str("%H:%M:%S%.3f DAE")
        .build();

    let level = get_level().unwrap_or(LevelFilter::Info);
    CombinedLogger::init(vec![
        TermLogger::new(level, config.clone(), TerminalMode::Stderr),
        WriteLogger::new(level, config, std::fs::File::create(log_file)?),
    ])
    .unwrap();

    log_panics::init();

    Ok(())
}

async fn main_async() -> anyhow::Result<()> {
    let (_tx, rx, _lifeline) = spawn().await?;
    wait_for_shutdown(rx).await;
    info!("PTY process terminated.");

    Ok(())
}

async fn spawn() -> anyhow::Result<(
    impl Sink<Item = PtyWebsocketRequest>,
    impl Stream<Item = MainShutdown>,
    MainService,
)> {
    let config = launch_daemon().await?;

    let bus = MainBus::default();
    bus.capacity::<PtyWebsocketRequest>(64)?;

    let ws_url = format!("ws://127.0.0.1:{}/pty", config.port);
    let websocket = tab_websocket::connect_authorized(ws_url, config.auth_token.clone()).await?;
    bus.store_resource(WebsocketResource(websocket));
    bus.store_resource(config);

    let main = MainService::spawn(&bus)?;

    let tx = bus.tx::<PtyWebsocketRequest>()?;
    let main_shutdown = bus.rx::<MainShutdown>()?;

    Ok((tx, main_shutdown, main))
}
