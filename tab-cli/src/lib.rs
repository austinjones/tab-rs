use clap::ArgMatches;

use crate::prelude::*;
use service::main::*;

use simplelog::{CombinedLogger, TermLogger, TerminalMode};

use crate::bus::MainBus;
use message::main::{MainRecv, MainShutdown};
use std::time::Duration;

use lifeline::dyn_bus::DynBus;
use tab_api::launch::*;
use tab_websocket::resource::connection::WebsocketResource;

mod bus;
mod message;
mod prelude;
mod service;
mod state;

pub fn cli_main(args: ArgMatches) -> anyhow::Result<()> {
    let mut runtime = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();

    let result = runtime.block_on(async { main_async(args).await });

    runtime.shutdown_timeout(Duration::from_millis(25));

    result?;

    Ok(())
}

async fn main_async(matches: ArgMatches<'_>) -> anyhow::Result<()> {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Warn,
        simplelog::Config::default(),
        TerminalMode::Stderr,
    )])
    .unwrap();

    let select_tab = matches.value_of("TAB-NAME");
    let (mut tx, rx_shutdown, _service) = spawn().await?;
    let completion = matches.is_present("AUTOCOMPLETE-TAB");
    let close = matches.is_present("CLOSE");
    let shutdown = matches.is_present("SHUTDOWN");

    if shutdown {
        tx.send(MainRecv::GlobalShutdown).await?;
    } else if completion {
        tx.send(MainRecv::AutocompleteTab).await?;
    } else if matches.is_present("LIST") {
        tx.send(MainRecv::ListTabs).await?;
    } else if let Some(tab) = select_tab {
        if close {
            tx.send(MainRecv::CloseTab(tab.to_string())).await?;
        } else {
            tx.send(MainRecv::SelectTab(tab.to_string())).await?;
        }
    } else {
        tx.send(MainRecv::SelectInteractive).await?;
    }

    wait_for_shutdown(rx_shutdown).await;

    Ok(())
}

async fn spawn() -> anyhow::Result<(
    impl Sender<MainRecv>,
    impl Receiver<MainShutdown>,
    MainService,
)> {
    let daemon_file = launch_daemon().await?;
    let ws_url = format!("ws://127.0.0.1:{}/cli", daemon_file.port);

    let bus = MainBus::default();

    let websocket =
        tab_websocket::connect_authorized(ws_url, daemon_file.auth_token.clone()).await?;
    let websocket = WebsocketResource(websocket);
    bus.store_resource(websocket);

    debug!("Launching MainService");
    let service = MainService::spawn(&bus)?;

    let tx = bus.tx::<MainRecv>()?;
    let main_shutdown = bus.rx::<MainShutdown>()?;

    Ok((tx, main_shutdown, service))
}
