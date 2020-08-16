use async_tungstenite::tokio::connect_async;
use clap::{App, Arg, ArgMatches, SubCommand};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use log::{info, LevelFilter};
use simplelog::{CombinedLogger, TermLogger, TerminalMode};
use std::time::Duration;
use tab_api::{
    config::load_daemon_file, request::Request, response::Response, tab::CreateTabMetadata,
};
use tab_websocket::{decode, decode_with, encode_with};
use tokio::{process::Command, time::delay_for};
use tungstenite::Message;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Starting.");

    let matches = init();
    run().await?;

    info!("Complete.  Shutting down");
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

async fn run() -> anyhow::Result<()> {
    info!("Loading daemon file");
    let daemon_file = load_daemon_file()?;
    if daemon_file.is_none() {
        info!("Starting daemon");
        start_daemon().await?;
    }

    while let None = load_daemon_file()? {
        delay_for(Duration::from_millis(25)).await;
    }

    info!("Connecting WebSocket");
    let daemon_file = load_daemon_file()?.expect("daemon file should be present");
    let server_url = format!("ws://127.0.0.1:{}", daemon_file.port);
    let (websocket, _) = connect_async(server_url).await?;

    let (tx, rx) = websocket.split();
    let mut tx = tx.with(|msg| encode_with(msg));
    let mut rx = rx.map(|msg| decode_with::<Response>(msg));

    tx.send(Request::Auth(vec![])).await?;
    tx.send(Request::ListTabs).await?;
    tx.send(Request::CreateTab(CreateTabMetadata {
        name: "foo".to_string(),
    }))
    .await?;

    info!("Waiting on messages...");
    while let Some(message) = rx.next().await {
        let message = message.await?;
        info!("message: {:?}", message);
    }

    Ok(())
}

async fn start_daemon() -> anyhow::Result<()> {
    Command::new("tab-daemon").spawn()?.await?;
    Ok(())
}
