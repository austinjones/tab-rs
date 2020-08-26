use crate::message::terminal::{TerminalRecv, TerminalSend, TerminalShutdown};
use crate::prelude::*;
use anyhow::Context;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
        let tx_shutdown = bus.tx::<TerminalShutdown>()?;

        let _output = Self::try_task("stdout", print_stdout(rx));

        let _input = Self::try_task("stdin", forward_stdin(tx, tx_shutdown));

        Ok(TerminalEchoService { _input, _output })
    }
}

impl Drop for TerminalEchoService {
    fn drop(&mut self) {
        disable_raw_mode().expect("failed to enable raw mode");
    }
}

async fn forward_stdin(
    mut tx: impl Sender<TerminalSend>,
    mut tx_shutdown: impl Sender<TerminalShutdown>,
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
            tx_shutdown.send(TerminalShutdown {}).await?;
            break;
        }

        trace!("stdin chunk of len {}", read);
        // TODO: use selected tab
        // TODO: better error handling for broadcast
        tx.send(TerminalSend::Stdin(buf))
            .await
            .context("tx TerminalSend::Stdin")?;
    }

    Ok(())
}

async fn print_stdout(mut rx: impl Receiver<TerminalRecv>) -> anyhow::Result<()> {
    trace!("Waiting on messages...");

    let mut stdout = tokio::io::stdout();

    while let Some(message) = rx.recv().await {
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
