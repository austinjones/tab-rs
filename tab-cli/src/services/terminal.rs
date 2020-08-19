use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use log::trace;
use std::io::Write;
use tab_service::{channel_tokio_mpsc, service_bus, spawn, Lifeline, Service};
use tokio::{io::AsyncReadExt, sync::mpsc};

#[derive(Debug)]
pub enum TerminalSend {
    Stdin(Vec<u8>),
}
pub enum TerminalRecv {
    Stdout(Vec<u8>),
}

service_bus!(pub TerminalBus);

channel_tokio_mpsc!(impl Channel<TerminalBus, 16> for TerminalSend);
channel_tokio_mpsc!(impl Channel<TerminalBus, 16> for TerminalRecv);

pub struct TerminalService {
    _input: Lifeline,
    _output: Lifeline,
}

impl Service for TerminalService {
    type Rx = mpsc::Receiver<TerminalRecv>;
    type Tx = mpsc::Sender<TerminalSend>;

    fn spawn(mut rx: Self::Rx, mut tx: Self::Tx) -> Self {
        let _output = spawn(print_stdout(rx));

        let _input = spawn(forward_stdin(tx));

        TerminalService { _input, _output }
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

        // TODO: use selected tab
        tx.send(TerminalSend::Stdin(buf)).await?;
    }

    trace!("forward_stdin shutdown");

    Ok(())
}

async fn print_stdout(mut rx: mpsc::Receiver<TerminalRecv>) -> anyhow::Result<()> {
    trace!("Waiting on messages...");

    let mut stdout = std::io::stdout();
    enable_raw_mode().expect("failed to enable raw mode");

    while let Some(message) = rx.recv().await {
        match message {
            TerminalRecv::Stdout(data) => {
                let mut index = 0;
                for line in data.split(|e| *e == b'\n') {
                    stdout.write(line)?;

                    index += line.len();
                    if index < data.len() {
                        let next = data[index];

                        if next == b'\n' {
                            stdout.write("\r\n".as_bytes())?;
                            index += 1;
                        }
                    }
                }

                stdout.flush()?;
            }
        }
    }

    disable_raw_mode().expect("failed to enable raw mode");

    trace!("recv_loop shutdown");

    Ok(())
}
