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
mod env;
mod message;
mod prelude;
mod service;
mod state;

pub fn command_main(args: ArgMatches) -> anyhow::Result<()> {
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
        simplelog::ConfigBuilder::new()
            .set_time_format("%+".to_owned())
            .build(),
        TerminalMode::Stderr,
    )])
    .unwrap();

    let select_tab = matches.value_of("TAB-NAME");
    let close_tab = matches.value_of("CLOSE-TAB");
    let (mut tx, rx_shutdown, _service) = spawn().await?;
    let completion = matches.is_present("AUTOCOMPLETE-TAB");
    let close_completion = matches.is_present("AUTOCOMPLETE-CLOSE-TAB");
    let shutdown = matches.is_present("SHUTDOWN");

    if shutdown {
        tx.send(MainRecv::GlobalShutdown).await?;
    } else if completion {
        tx.send(MainRecv::AutocompleteTab).await?;
    } else if close_completion {
        tx.send(MainRecv::AutocompleteCloseTab).await?;
    } else if matches.is_present("LIST") {
        tx.send(MainRecv::ListTabs).await?;
    } else if let Some(tab) = select_tab {
        info!("selecting tab: {}", tab);
        tx.send(MainRecv::SelectTab(tab.to_string())).await?;
    } else if let Some(tab) = close_tab {
        tx.send(MainRecv::CloseTab(tab.to_string())).await?;
    } else {
        tx.send(MainRecv::SelectTab("any/".to_string())).await?;
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
    bus.capacity::<Request>(128)?;
    bus.capacity::<Response>(256)?;

    let websocket =
        tab_websocket::connect_authorized(ws_url, daemon_file.auth_token.clone()).await?;
    let websocket = WebsocketResource(websocket);
    bus.store_resource(websocket);

    info!("Launching MainService");
    let service = MainService::spawn(&bus)?;

    let tx = bus.tx::<MainRecv>()?;
    let main_shutdown = bus.rx::<MainShutdown>()?;

    Ok((tx, main_shutdown, service))
}

pub fn normalize_name(name: &str) -> String {
    let name = name.to_string().trim().to_string();
    if name.ends_with("/") {
        name
    } else {
        name + "/"
    }
}
