use crate::bus::TerminalBus;
use crate::message::{
    main::MainShutdown,
    terminal::{TerminalRecv, TerminalSend},
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use log::trace;
use tab_service::{Bus, Lifeline, Service};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{broadcast, mpsc},
};

pub struct TerminalEchoService {
    _input: Lifeline,
    _output: Lifeline,
}

impl Service for TerminalEchoService {
    type Bus = TerminalBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &TerminalBus) -> anyhow::Result<Self> {
        enable_raw_mode().expect("failed to enable raw mode");

        let rx = bus.rx::<TerminalRecv>()?;
        let tx = bus.tx::<TerminalSend>()?;
        let tx_shutdown = bus.tx::<MainShutdown>()?;

        let _output = Self::try_task("stdout", print_stdout(rx));

        let _input = Self::try_task("stdin", forward_stdin(tx, tx_shutdown));

        // let event_tx = TerminalEventTx { size: tx.size };

        Ok(TerminalEchoService { _input, _output })
    }
}

impl Drop for TerminalEchoService {
    fn drop(&mut self) {
        disable_raw_mode().expect("failed to enable raw mode");
    }
}

async fn forward_stdin(
    tx: broadcast::Sender<TerminalSend>,
    mut tx_shutdown: mpsc::Sender<MainShutdown>,
) -> anyhow::Result<()> {
    let mut stdin = tokio::io::stdin();
    let mut buffer = vec![0u8; 512];

    while let Ok(read) = stdin.read(buffer.as_mut_slice()).await {
        if read == 0 {
            continue;
        }

        let mut buf = vec![0; read];
        buf.copy_from_slice(&buffer[0..read]);

        // this is ctrl-w
        if buf.contains(&23u8) {
            tx_shutdown.send(MainShutdown {}).await?;
            break;
        }

        trace!("stdin chunk of len {}", read);
        // TODO: use selected tab
        // TODO: better error handling for broadcast
        tx.send(TerminalSend::Stdin(buf))
            .map_err(|_| anyhow::Error::msg("tx TerminalSend::Stdin"))?;
    }

    Ok(())
}

async fn print_stdout(mut rx: broadcast::Receiver<TerminalRecv>) -> anyhow::Result<()> {
    trace!("Waiting on messages...");

    let mut stdout = tokio::io::stdout();

    while let Ok(message) = rx.recv().await {
        match message {
            TerminalRecv::Stdout(data) => {
                if data.len() == 0 {
                    continue;
                }

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
