use clap::{App, Arg, ArgMatches};

use log::{error, info, LevelFilter};
use service::main::*;

use simplelog::{CombinedLogger, TermLogger, TerminalMode};

use crate::bus::MainBus;
use message::main::{MainRecv, MainShutdown};
use std::time::Duration;
use tab_api::config::load_daemon_file;
use tab_service::{dyn_bus::DynBus, Bus, Service};

use tab_websocket::resource::connection::WebsocketResource;
use tokio::{
    select,
    signal::ctrl_c,
    sync::{mpsc, oneshot},
};

mod bus;
mod message;
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
            Arg::with_name("TAB")
                .help("Switches to the provided tab")
                .required(false)
                .index(1),
        )
        .arg(
            Arg::with_name("command")
                .short("c")
                .possible_values(&["list", "_autocomplete-tab"])
                .help("print debug information verbosely"),
        )
        .get_matches()
}

async fn main_async() -> anyhow::Result<()> {
    println!("Starting.");

    let matches = init();
    let select_tab = matches.value_of("TAB");
    let (mut tx, shutdown, service) = spawn().await?;

    if let Some(tab) = select_tab {
        tx.send(MainRecv::SelectTab(tab.to_string())).await?;
    } else {
        tx.send(MainRecv::SelectInteractive).await?;
    }

    wait_for_shutdown(shutdown).await;

    Ok(())
}

async fn spawn() -> anyhow::Result<(
    mpsc::Sender<MainRecv>,
    oneshot::Receiver<MainShutdown>,
    MainService,
)> {
    let daemon_file = load_daemon_file()?.unwrap();
    let ws_url = format!("ws://127.0.0.1:{}", daemon_file.port);

    let bus = MainBus::default();

    let websocket = tab_websocket::connect(ws_url).await?;
    let websocket = WebsocketResource(websocket);
    bus.store_resource(websocket);

    info!("Launching MainService");
    let service = MainService::spawn(&bus)?;

    let mut tx = bus.tx::<MainRecv>()?;
    let main_shutdown = bus.rx::<MainShutdown>()?;

    Ok((tx, main_shutdown, service))
}

async fn wait_for_shutdown(receiver: oneshot::Receiver<MainShutdown>) {
    info!("Waiting for termination");

    loop {
        select! {
            _ = ctrl_c() => {
                break;
            },
            _ = receiver => {
                break;
            }
        }
    }

    info!("Complete.  Shutting down");
}
async fn start_daemon() -> anyhow::Result<()> {
    // Command::new("tab-daemon").spawn()?.await?;
    Ok(())
}
