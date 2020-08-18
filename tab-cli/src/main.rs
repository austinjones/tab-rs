use async_tungstenite::tokio::connect_async;
use clap::{App, Arg, ArgMatches};
use crossterm::terminal::{enable_raw_mode, size};
use futures::sink::SinkExt;
use futures::{
    future::{ready, AbortHandle, Abortable},
    stream::StreamExt,
    Future, Sink, Stream,
};
use log::{info, trace, LevelFilter};
use simplelog::{CombinedLogger, TermLogger, TerminalMode};
use state::ClientState;
use std::{io::Write, time::Duration};
use tab_api::{
    chunk::InputChunk,
    config::load_daemon_file,
    request::Request,
    response::Response,
    tab::{CreateTabMetadata, TabId},
};
use tab_websocket::{client::spawn_client, decode_with, encode, encode_or_close, encode_with};
use tokio::io::AsyncReadExt;
use tokio::{
    runtime::Runtime,
    sync::{
        mpsc::{Receiver, Sender},
        oneshot, watch,
    },
    time::delay_for,
};
use tungstenite::Message;
mod service;
mod services;
mod state;

pub fn main() -> anyhow::Result<()> {
    let mut runtime = Runtime::new()?;

    runtime.block_on(main_async())?;
    runtime.shutdown_timeout(Duration::from_millis(250));

    Ok(())
}

async fn main_async() -> anyhow::Result<()> {
    println!("Starting.");

    let _matches = init();
    // run_tab("foo").await?;

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

async fn run_tab(name: String) -> anyhow::Result<()> {
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

    // let (tx, rx) = spawn_client(server_url.as_str(), Request::is_close).await?;
    // let mut tx_close = tx.clone();
    // let (websocket, _) = connect_async(server_url).await?;

    // let (tx, rx) = websocket.split();
    // let tx = tx.with(|msg: Request| ready(encode_or_close(msg, Request::is_close)));

    // let rx = rx.map(|msg| decode_with::<Response>(msg));

    let state = ClientState::default();
    // tokio::spawn(send_loop(tx.clone()));
    // recv_loop(tx, rx).await?;

    // tx_close.send(Request::Close).await?;

    Ok(())
}

// async fn send_loop(mut tx: Sender<Request>) -> anyhow::Result<()> {
//     tx.send(Request::Auth(vec![])).await?;
//     tx.send(Request::ListTabs).await?;
//     tx.send(Request::CreateTab(CreateTabMetadata {
//         name: "foo".to_string(),
//         dimensions: size()?,
//     }))
//     .await?;

//     forward_stdin(tx).await?;

//     trace!("send_loop shutdown");

//     Ok(())
// }

async fn start_daemon() -> anyhow::Result<()> {
    // Command::new("tab-daemon").spawn()?.await?;
    Ok(())
}
