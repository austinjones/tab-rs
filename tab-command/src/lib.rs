use std::path::PathBuf;

use clap::ArgMatches;
use semver::Version;

use crate::prelude::*;
use service::{main::*, terminal::disable_raw_mode, terminal::reset_cursor};

use simplelog::{TermLogger, TerminalMode};

use crate::bus::MainBus;
use message::main::{MainRecv, MainShutdown};

use lifeline::dyn_bus::DynBus;
use tab_api::{config::DaemonConfig, launch::*, log::get_level, tab::normalize_name};
use tab_websocket::resource::connection::WebsocketResource;

mod bus;
mod env;
mod message;
mod prelude;
mod service;
mod state;

pub fn command_main(args: ArgMatches, tab_version: &'static str) -> anyhow::Result<()> {
    TermLogger::init(
        get_level().unwrap_or(LevelFilter::Warn),
        simplelog::ConfigBuilder::new()
            .set_time_format_str("%H:%M:%S%.3f CMD")
            .build(),
        TerminalMode::Stderr,
    )
    .unwrap();

    info!("tab-command runtime starting");

    let mut runtime = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();

    let result = runtime.block_on(async { main_async(args, tab_version).await });

    runtime.shutdown_background();

    result?;

    info!("tab-command runtime stopped");

    Ok(())
}

async fn main_async(matches: ArgMatches<'_>, tab_version: &'static str) -> anyhow::Result<()> {
    let select_tab = matches.value_of("TAB-NAME");
    let close_tabs = matches.values_of("CLOSE-TAB");
    let (mut tx, rx_shutdown, _service) = spawn(tab_version).await?;
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
    } else if let Some(tabs) = close_tabs {
        let tabs: Vec<String> = tabs.map(normalize_name).collect();
        tx.send(MainRecv::CloseTabs(tabs)).await?;
    } else {
        tx.send(MainRecv::SelectTab("any/".to_string())).await?;
    }

    wait_for_shutdown(rx_shutdown).await;
    disable_raw_mode();
    reset_cursor();

    Ok(())
}

async fn spawn(
    tab_version: &'static str,
) -> anyhow::Result<(
    impl Sender<MainRecv>,
    impl Receiver<MainShutdown>,
    MainService,
)> {
    let daemon_file = launch_daemon().await?;
    validate_daemon(&daemon_file, tab_version);
    let ws_url = format!("ws://127.0.0.1:{}/cli", daemon_file.port);

    debug!("daemon is ready");

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

fn validate_daemon(config: &DaemonConfig, tab_version: &'static str) {
    let executable = std::env::current_exe()
        .ok()
        .map(|path| path.to_str().map(str::to_string))
        .flatten();

    let tab_version = Version::parse(tab_version).ok();
    let daemon_version = config
        .tab_version
        .as_ref()
        .map(String::as_str)
        .map(Version::parse)
        .map(Result::ok)
        .flatten();

    if let (Some(tab_version), Some(daemon_version)) = (tab_version, daemon_version) {
        if tab_version.major != daemon_version.major || tab_version.minor != daemon_version.minor {
            eprintln!("Warning: The tab command (v{}) has an incompatible version with the running daemon (v{})", tab_version, daemon_version);
            eprintln!(
                "  You should run `tab --shutdown` to terminate your tabs and relaunch the daemon."
            );

            return;
        }
    }

    if let (Some(executable), Some(daemon_exec)) = (&executable, &config.executable) {
        if executable != daemon_exec {
            eprintln!(
                "Warning: The tab command has a different executable path than the running daemon."
            );
            eprintln!("  You may want to run `tab --shutdown` to relaunch the daemon.");

            eprintln!("  Tab command: {}", executable);
            eprintln!("  Daemon command: {}", daemon_exec);
            return;
        }
    }
}
