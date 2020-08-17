use async_tungstenite::{tokio::TokioAdapter, WebSocketStream};
use daemonfile::DaemonFile;
use endpoint::handle_request;
use futures::sink::SinkExt;
use futures::stream::{SplitSink, StreamExt};
use log::{error, info, LevelFilter};
use runtime::DaemonRuntime;
use session::DaemonSession;
use simplelog::{CombinedLogger, TermLogger, TerminalMode, WriteLogger};
use std::{sync::Arc, time::Duration};
use tab_api::{
    config::{daemon_log, DaemonConfig},
    request::Request,
    response::Response,
};
use tab_websocket::{decode, encode, encode_or_close, server::spawn_server};
use tokio::{
    net::{TcpListener, TcpStream},
    task,
    time::delay_for,
};
use tungstenite::{Error, Message};

mod daemonfile;
mod endpoint;
mod pty_process;
mod runtime;
mod session;

#[tokio::main(max_threads = 32)]
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
                    // task::spawn(accept_connection(runtime.clone(), stream));
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

    info!("connection opened from `{}`", addr);

    let mut session = DaemonSession::new(runtime);
    let (mut rx_request, tx_response) = spawn_server(stream, Response::is_close).await?;

    while let Some(msg) = rx_request.recv().await {
        handle_request(msg, &mut session, tx_response.clone()).await?
    }

    info!("connection closed from `{}`", addr);

    Ok(())
}

/// Convert the socket into a mpsc sender.  This allows asynchronous subscriptions to stdin/stderr
async fn process_responses(
    mut socket: SplitSink<WebSocketStream<TokioAdapter<TcpStream>>, Message>,
) -> tokio::sync::mpsc::Sender<Response> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    task::spawn(async move {
        while let Some(message) = rx.next().await {
            info!("send message: {:?}", message);
            // TODO: log errors
            let serialized_message = encode_or_close(message, Response::is_close).unwrap();
            match socket.send(serialized_message).await {
                Ok(_) => {}
                Err(Error::ConnectionClosed) => {}
                Err(e) => panic!(format!("websocket send error: {}", e)),
            }
        }

        info!("response processor shutdown");
    });

    tx
}
