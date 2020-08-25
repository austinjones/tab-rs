use clap::{App, Arg, ArgMatches};

use crate::prelude::*;
use service::main::*;

use simplelog::{CombinedLogger, TermLogger, TerminalMode};

use crate::bus::MainBus;
use message::main::{MainRecv, MainShutdown};
use std::{
    process::Stdio,
    time::{Duration, Instant},
};

use dyn_bus::DynBus;
use tab_api::launch::*;
use tab_websocket::resource::connection::WebsocketResource;
use tokio::{
    process::Command,
    select,
    signal::ctrl_c,
    sync::{broadcast, mpsc},
    time,
};

mod bus;
mod message;
mod prelude;
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

fn init() -> ArgMatches<'static> {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Debug,
        simplelog::Config::default(),
        TerminalMode::Stderr,
    )])
    .unwrap();

    App::new("Terminal Multiplexer")
        .version("0.1")
        .author("Austin Jones <implAustin@gmail.com>")
        .about("Provides persistent terminal sessions with multiplexing.")
        .arg(
            Arg::with_name("DEV")
                .long("dev")
                .required(false)
                .takes_value(false)
                .help("runs the daemon using `cargo run`"),
        )
        .arg(
            Arg::with_name("COMPLETION")
                .long("_completion")
                .takes_value(true)
                .help("runs the daemon using `cargo run`"),
        )
        .arg(
            Arg::with_name("CLOSE")
                .short("w")
                .takes_value(false)
                .help("print debug information verbosely"),
        )
        .arg(
            Arg::with_name("LIST")
                .short("l")
                .help("lists all the active tabs"),
        )
        .arg(
            Arg::with_name("TAB")
                .help("Switches to the provided tab")
                .required(false)
                .index(1),
        )
        .get_matches()
}

async fn main_async() -> anyhow::Result<()> {
    let matches = init();
    let select_tab = matches.value_of("TAB");
    let dev = matches.is_present("DEV");
    let (tx, shutdown, _service) = spawn(dev).await?;
    let completion = matches.value_of("COMPLETION");
    let close = matches.is_present("CLOSE");

    if let Some(comp) = completion {
        tx.send(MainRecv::AutocompleteTab(comp.to_string()))
            .map_err(into_msg)?;
    } else if matches.is_present("LIST") {
        tx.send(MainRecv::ListTabs).map_err(into_msg)?;
    } else if let Some(tab) = select_tab {
        if close {
            tx.send(MainRecv::CloseTab(tab.to_string()))
                .map_err(into_msg)?;
        } else {
            tx.send(MainRecv::SelectTab(tab.to_string()))
                .map_err(into_msg)?;
        }
    } else {
        tx.send(MainRecv::SelectInteractive).map_err(into_msg)?;
    }

    wait_for_shutdown(shutdown).await;

    Ok(())
}

async fn spawn(
    dev: bool,
) -> anyhow::Result<(
    broadcast::Sender<MainRecv>,
    mpsc::Receiver<MainShutdown>,
    MainService,
)> {
    let daemon_file = launch_daemon(dev).await?;
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
