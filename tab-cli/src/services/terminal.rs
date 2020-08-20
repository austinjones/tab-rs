use super::terminal_event::TerminalEventService;
use crate::bus::client::ClientBus;
use crate::message::terminal::{TerminalRecv, TerminalSend};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use log::trace;
use tab_service::{Bus, Lifeline, Service};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
};

pub struct TerminalService {
    _input: Lifeline,
    _output: Lifeline,
    _events: TerminalEventService,
}

impl Service for TerminalService {
    type Bus = ClientBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &ClientBus) -> anyhow::Result<Self> {
        enable_raw_mode().expect("failed to enable raw mode");

        let rx = bus.rx::<TerminalRecv>()?;
        let tx = bus.tx::<TerminalSend>()?;

        let _output = Self::task("stdout", print_stdout(rx));

        let _input = Self::task("stdin", forward_stdin(tx));

        // let event_tx = TerminalEventTx { size: tx.size };
        let _events = TerminalEventService::spawn(bus)?;

        Ok(TerminalService {
            _input,
            _output,
            _events,
        })
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
