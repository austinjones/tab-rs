use clap::{App, Arg, ArgMatches};

use log::{error, info, LevelFilter};
use service::main::*;

use simplelog::{CombinedLogger, TermLogger, TerminalMode};

use crate::bus::main::MainBus;
use message::main::{MainRecv, MainShutdown};
use std::time::Duration;
use tab_api::config::load_daemon_file;
use tab_service::{dyn_bus::DynBus, Bus, Service};
use tab_websocket::service::WebsocketResource;

use tokio::{select, signal::ctrl_c};

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

    runtime.block_on(async {
        match main_async().await {
            Ok(()) => {}
            Err(e) => error!("fatal error: {}", e),
        };
    });
    runtime.shutdown_timeout(Duration::from_millis(25));

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

    let _matches = init();

    let daemon_file = load_daemon_file()?.unwrap();
    let ws_url = format!("ws://127.0.0.1:{}", daemon_file.port);

    let bus = MainBus::default();

    let websocket = tab_websocket::connect(ws_url).await?;
    let websocket = WebsocketResource(websocket);
    bus.store_resource(websocket);

    info!("Launching MainService");
    let _service = MainService::spawn(&bus)?;

    let mut tx = bus.tx::<MainRecv>()?;
    tx.send(MainRecv::SelectTab("tabby".to_string())).await?;

    let main_shutdown = bus.rx::<MainShutdown>()?;

    info!("Waiting for termination");
    loop {
        select! {
            _ = ctrl_c() => {
                break;
            },
            _ = main_shutdown => {
                break;
            }
        }
    }

    info!("Complete.  Shutting down");
    Ok(())
}

async fn start_daemon() -> anyhow::Result<()> {
    // Command::new("tab-daemon").spawn()?.await?;
    Ok(())
}
