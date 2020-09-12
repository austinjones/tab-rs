use crate::prelude::*;

use message::pty::MainShutdown;
use simplelog::{CombinedLogger, TermLogger, TerminalMode};
use std::time::Duration;
use tab_api::{launch::*, pty::PtyWebsocketRequest};

use lifeline::dyn_bus::DynBus;
use service::main::MainService;
use tab_websocket::resource::connection::WebsocketResource;

mod bus;
mod message;
mod prelude;
mod service;

pub fn pty_main() -> anyhow::Result<()> {
    debug!("pty process started");
    let mut runtime = tokio::runtime::Builder::new()
        .threaded_scheduler()
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

fn init() {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        simplelog::ConfigBuilder::new()
            .set_time_format_str("%H:%M:%S%.3f PTY")
            .build(),
        TerminalMode::Stderr,
    )])
    .unwrap();
}

async fn main_async() -> anyhow::Result<()> {
    let _matches = init();

    let (_tx, rx, _lifeline) = spawn().await?;
    wait_for_shutdown(rx).await;

    Ok(())
}

async fn spawn() -> anyhow::Result<(
    impl Sender<PtyWebsocketRequest>,
    impl Receiver<MainShutdown>,
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
