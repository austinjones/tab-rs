use async_tungstenite::tokio::connect_async;
use clap::{App, Arg, ArgMatches};
use crossterm::{
    terminal::{enable_raw_mode, size},
};
use futures::sink::SinkExt;
use futures::{stream::StreamExt, Future, Sink, Stream};
use log::{info, LevelFilter};
use simplelog::{CombinedLogger, TermLogger, TerminalMode};
use std::{io::Write, time::Duration};
use tab_api::{
    chunk::{ChunkType, StdinChunk},
    config::load_daemon_file,
    request::Request,
    response::Response,
    tab::{CreateTabMetadata, TabId},
};
use tab_websocket::{decode_with, encode_with};
use tokio::io::{AsyncReadExt};
use tokio::time::delay_for;


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Starting.");

    let _matches = init();
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
    let tx = tx.with(|msg| encode_with(msg));
    let rx = rx.map(|msg| decode_with::<Response>(msg));
    tokio::spawn(send_loop(tx));

    recv_loop(rx).await?;

    Ok(())
}

async fn send_loop(
    mut tx: impl Sink<Request, Error = anyhow::Error> + Unpin,
) -> anyhow::Result<()> {
    tx.send(Request::Auth(vec![])).await?;
    tx.send(Request::ListTabs).await?;
    tx.send(Request::CreateTab(CreateTabMetadata {
        name: "foo".to_string(),
        dimensions: size()?,
    }))
    .await?;

    forward_stdin(tx).await?;

    Ok(())
}

async fn forward_stdin(
    mut tx: impl Sink<Request, Error = anyhow::Error> + Unpin,
) -> anyhow::Result<()> {
    let mut stdin = tokio::io::stdin();
    let mut buffer = vec![0u8; 512];

    while let Ok(read) = stdin.read(buffer.as_mut_slice()).await {
        if read == 0 {
            continue;
        }

        let mut buf = vec![0; read];
        buf.copy_from_slice(&buffer[0..read]);

        let chunk = StdinChunk { data: buf };
        // TODO: use selected tab
        tx.send(Request::Stdin(TabId(0), chunk)).await?;
    }

    Ok(())
}

async fn recv_loop(
    mut rx: impl Stream<Item = impl Future<Output = anyhow::Result<Response>>> + Unpin,
) -> anyhow::Result<()> {
    info!("Waiting on messages...");

    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    enable_raw_mode().expect("failed to enable raw mode");

    while let Some(message) = rx.next().await {
        let message = message.await?;
        // info!("message: {:?}", message);

        match message {
            Response::Chunk(_tab_id, chunk) => match chunk.channel {
                ChunkType::Stdout => {
                    let mut index = 0;
                    for line in chunk.data.split(|e| *e == b'\n') {
                        stdout.write(line)?;

                        index += line.len();
                        if index < chunk.data.len() {
                            let next = chunk.data[index];

                            if next == b'\n' {
                                stdout.write("\r\n".as_bytes())?;
                                index += 1;
                            }
                        }
                    }

                    stdout.flush()?;
                }
                ChunkType::Stderr => {
                    stderr.write_all(chunk.data.as_slice())?;
                }
            },
            Response::TabUpdate(_tab) => {}
            Response::TabList(_tabs) => {}
        }
    }

    Ok(())
}

async fn start_daemon() -> anyhow::Result<()> {
    // Command::new("tab-daemon").spawn()?.await?;
    Ok(())
}
