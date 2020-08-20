use super::state::TerminalSizeState;
use crossterm::{
    event::{poll, Event},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use log::{debug, info, trace};
use std::{io::Write, time::Duration};
use tab_service::{service_bus, Lifeline, Message, Service};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, watch},
};

#[derive(Debug)]
pub enum TerminalSend {
    Stdin(Vec<u8>),
}

#[derive(Debug)]
pub enum TerminalRecv {
    Stdout(Vec<u8>),
}

service_bus!(pub TerminalBus);

impl Message<TerminalBus> for TerminalSend {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TerminalBus> for TerminalRecv {
    type Channel = mpsc::Sender<Self>;
}

pub struct TerminalService {
    _input: Lifeline,
    _output: Lifeline,
    _events: Lifeline,
}

pub struct TerminalTx {
    pub tx: mpsc::Sender<TerminalSend>,
    pub size: watch::Sender<TerminalSizeState>,
}

impl Service for TerminalService {
    type Rx = mpsc::Receiver<TerminalRecv>;
    type Tx = TerminalTx;
    type Lifeline = Self;

    fn spawn(mut rx: Self::Rx, mut tx: Self::Tx) -> Self {
        enable_raw_mode().expect("failed to enable raw mode");
        let _output = Self::task("stdout", print_stdout(rx));

        let _input = Self::task("stdin", forward_stdin(tx.tx));

        let event_tx = TerminalEventTx { size: tx.size };
        let _events = TerminalEventService::spawn((), event_tx);

        TerminalService {
            _input,
            _output,
            _events,
        }
    }
}

async fn forward_stdin(mut tx: mpsc::Sender<TerminalSend>) -> anyhow::Result<()> {
    let mut stdin = tokio::io::stdin();
    let mut buffer = vec![0u8; 512];

    while let Ok(read) = stdin.read(buffer.as_mut_slice()).await {
        if read == 0 {
            continue;
        }

        let mut buf = vec![0; read];
        buf.copy_from_slice(&buffer[0..read]);

        trace!("stdin chunk of len {}", read);
        // TODO: use selected tab
        tx.send(TerminalSend::Stdin(buf)).await?;
    }

    Ok(())
}

async fn print_stdout(mut rx: mpsc::Receiver<TerminalRecv>) -> anyhow::Result<()> {
    trace!("Waiting on messages...");

    let mut stdout = tokio::io::stdout();

    while let Some(message) = rx.recv().await {
        match message {
            TerminalRecv::Stdout(data) => {
                trace!("stdout chunk of len {}", data.len());
                let mut index = 0;
                for line in data.split(|e| *e == b'\n') {
                    stdout.write(line).await?;

                    index += line.len();
                    if index < data.len() {
                        let next = data[index];

                        if next == b'\n' {
                            stdout.write("\r\n".as_bytes()).await?;
                            index += 1;
                        }
                    }
                }

                stdout.flush().await?;
            }
        }
    }

    disable_raw_mode().expect("failed to enable raw mode");

    Ok(())
}

pub struct TerminalEventService {
    _update: Lifeline,
}

pub struct TerminalEventTx {
    size: watch::Sender<TerminalSizeState>,
}

impl Service for TerminalEventService {
    type Rx = ();
    type Tx = TerminalEventTx;
    type Lifeline = Lifeline;

    fn spawn(rx: (), mut tx: Self::Tx) -> Self::Lifeline {
        Self::task("run", async move {
            let mut size = crossterm::terminal::size().expect("get terminal size");
            tx.size
                .broadcast(TerminalSizeState(size))
                .expect("failed to send terminal size");

            // loop {
            //     let new_size = crossterm::terminal::size().expect("get terminal size");
            //     let msg = tokio::task::spawn_blocking(|| block_for_event())
            //         .await
            //         .expect("failed to get crossterm event");

            //     if !msg.is_some() {
            //         continue;
            //     }

            //     if let Event::Resize(width, height) = msg.unwrap() {
            //         let new_size = (height, width);
            //         if new_size != size {
            //             size = new_size;
            //             tx.size
            //                 .broadcast(TerminalSizeState(new_size))
            //                 .expect("send terminal size");
            //         }
            //     }
            // }
        })
    }
}

fn block_for_event() -> Option<Event> {
    if crossterm::event::poll(Duration::from_millis(500)).unwrap_or(false) {
        crossterm::event::read().ok()
    } else {
        None
    }
}
