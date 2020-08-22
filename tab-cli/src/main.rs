use clap::{App, Arg, ArgMatches};

use log::{debug, info, LevelFilter};
use service::main::*;

use simplelog::{CombinedLogger, TermLogger, TerminalMode};

use crate::bus::MainBus;
use message::main::{MainRecv, MainShutdown};
use std::{
    process::Stdio,
    time::{Duration, Instant},
};
use tab_api::config::{is_running, load_daemon_file, DaemonConfig};
use tab_service::{dyn_bus::DynBus, Bus, Service};

use tab_websocket::resource::connection::WebsocketResource;
use tokio::{process::Command, select, signal::ctrl_c, sync::mpsc, time};

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
            Arg::with_name("DEV")
                .long("dev")
                .required(false)
                .takes_value(false)
                .help("runs the daemon using `cargo run`"),
        )
        .arg(
            Arg::with_name("COMMAND")
                .short("c")
                .possible_values(&["list", "_autocomplete-tab"])
                .help("print debug information verbosely"),
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
    println!("Starting.");

    let matches = init();
    let select_tab = matches.value_of("TAB");
    let dev = matches.is_present("DEV");
    let (mut tx, shutdown, _service) = spawn(dev).await?;

    if let Some(tab) = select_tab {
        tx.send(MainRecv::SelectTab(tab.to_string())).await?;
    } else {
        tx.send(MainRecv::SelectInteractive).await?;
    }

    wait_for_shutdown(shutdown).await;

    Ok(())
}

async fn spawn(
    dev: bool,
) -> anyhow::Result<(
    mpsc::Sender<MainRecv>,
    mpsc::Receiver<MainShutdown>,
    MainService,
)> {
    let daemon_file = launch_daemon(dev).await?;
    let ws_url = format!("ws://127.0.0.1:{}", daemon_file.port);

    let bus = MainBus::default();

    let websocket = tab_websocket::connect(ws_url).await?;
    let websocket = WebsocketResource(websocket);
    bus.store_resource(websocket);

    debug!("Launching MainService");
    let service = MainService::spawn(&bus)?;

    let tx = bus.tx::<MainRecv>()?;
    let main_shutdown = bus.rx::<MainShutdown>()?;

    Ok((tx, main_shutdown, service))
}

async fn wait_for_shutdown(mut receiver: mpsc::Receiver<MainShutdown>) {
    info!("Waiting for termination");

    loop {
        select! {
            _ = ctrl_c() => {
                break;
            },
            _ = receiver.recv() => {
                break;
            }
        }
    }

    info!("Complete.  Shutting down");
}

async fn launch_daemon(dev: bool) -> anyhow::Result<DaemonConfig> {
    let daemon_file = load_daemon_file()?;

    let running = daemon_file
        .as_ref()
        .map(|config| is_running(config))
        .unwrap_or(false);

    let start_wait = Instant::now();
    if !running {
        if dev {
            info!("launching daemon using `cargo`");
            let _child = Command::new("cargo")
                .args(&["run", "--bin", "tab-daemon"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(false)
                .spawn()?;
        } else {
            debug!("launching daemon using `env`");
            let _child = Command::new("/usr/bin/env")
                .arg("tab-daemon")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(false)
                .spawn()?;
        };
    }

    let mut index = 0;
    let daemon_file = loop {
        if let Some(daemon_file) = load_daemon_file()? {
            if is_running(&daemon_file) {
                break daemon_file;
            }
        }

        time::delay_for(Duration::from_millis(50)).await;
        if Instant::now().duration_since(start_wait) > Duration::from_secs(2) {
            return Err(anyhow::Error::msg("timeout while waiting for tab daemon"));
        }

        if index == 1 {
            info!("waiting for daemon...");
        }

        index += 1;
    };

    Ok(daemon_file)
}
