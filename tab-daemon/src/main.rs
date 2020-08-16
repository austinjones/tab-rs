use async_tungstenite::{tokio::TokioAdapter, WebSocketStream};
use daemonfile::DaemonFile;
use endpoint::handle_request;
use futures::sink::SinkExt;
use futures::{
    stream::{SplitSink, StreamExt},
    Sink,
};
use log::{error, info, LevelFilter};
use runtime::DaemonRuntime;
use session::DaemonSession;
use simplelog::{CombinedLogger, TermLogger, TerminalMode, WriteLogger};
use std::{borrow::Borrow, cell::RefCell, rc::Rc, sync::Arc, time::Duration};
use tab_api::{
    config::{daemon_log, DaemonConfig},
    request::Request,
    response::Response,
};
use tab_websocket::{decode, encode, encode_with};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::Sender,
    task,
};
use tungstenite::Message;

mod daemonfile;
mod endpoint;
mod runtime;
mod session;

#[tokio::main(core_threads = 4, max_threads = 16)]
async fn main() -> anyhow::Result<()> {
    let log_file = daemon_log()?;

    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Debug,
            simplelog::Config::default(),
            TerminalMode::Stderr,
        ),
        WriteLogger::new(
            LevelFilter::Info,
            simplelog::Config::default(),
            std::fs::File::create(log_file)?,
        ),
    ])
    .unwrap();

    let mut server = TcpListener::bind("127.0.0.1:0").await?;
    let port = server.local_addr()?.port();

    let pid = std::process::id();
    let config = DaemonConfig { pid, port };

    let daemon_file = DaemonFile::new(&config)?;
    info!("Daemon started.");
    info!("Daemon pid: {}", pid);
    info!("Daemon port: {}", port);

    let runtime = Arc::new(DaemonRuntime::new());
    task::spawn(async move {
        let runtime = runtime.clone();
        loop {
            info!("waiting for connection.");
            let connect = server.accept().await;
            match connect {
                Ok((stream, _addr)) => {
                    // TODO: only accept connections from loopback address
                    info!("connection opened from {:?}", _addr);
                    task::spawn(accept_connection(runtime.clone(), stream));
                }
                Err(e) => {
                    error!("tcp connection failed: {}", e);
                    break;
                }
            }
        }
    });

    // TODO: intelligent shutdown behavior
    tokio::time::delay_for(Duration::from_millis(60000)).await;

    info!("tab daemon shutting down...");
    drop(daemon_file);

    Ok(())
}

async fn accept_connection(runtime: Arc<DaemonRuntime>, stream: TcpStream) -> anyhow::Result<()> {
    let addr = stream.peer_addr()?;
    let websocket = async_tungstenite::tokio::accept_async(stream).await?;
    let (tx, mut rx) = websocket.split();

    let tx = process_responses(tx).await;
    // let mut websocket = parse_bincode(websocket);
    info!("connection opened from `{}`", addr);

    let mut session = DaemonSession::new(runtime);

    while let Some(msg) = rx.next().await {
        let msg = decode(msg)?;
        handle_request(msg, &mut session, tx.clone()).await?
    }

    Ok(())
}

/// Convert the socket into a mpsc sender.  This allows asynchronous subscriptions to stdin/stderr
async fn process_responses(
    mut socket: SplitSink<WebSocketStream<TokioAdapter<TcpStream>>, Message>,
) -> tokio::sync::mpsc::Sender<Response> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    task::spawn(async move {
        while let Some(message) = rx.next().await {
            // TODO: log errors
            let serialized_message = encode(message).unwrap();
            socket.send(serialized_message).await.unwrap();
        }
    });

    tx
}
