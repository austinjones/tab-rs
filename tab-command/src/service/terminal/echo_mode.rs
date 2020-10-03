use std::sync::atomic::{AtomicBool, Ordering};

use crate::message::terminal::{TerminalRecv, TerminalSend, TerminalShutdown};
use crate::prelude::*;
use anyhow::Context;
use crossterm::execute;
use tab_api::env::is_raw_mode;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

static RAW_MODE_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn enable_raw_mode() {
    if is_raw_mode() {
        crossterm::terminal::enable_raw_mode().expect("failed to enable raw mode");
        RAW_MODE_ENABLED.store(true, Ordering::SeqCst);
        debug!("raw mode enabled");
    }
}

pub fn disable_raw_mode() {
    if is_raw_mode() && RAW_MODE_ENABLED.load(Ordering::SeqCst) {
        crossterm::terminal::disable_raw_mode().expect("failed to disable raw mode");
        debug!("raw mode disabled");
    }
}

pub fn reset_cursor() {
    if is_raw_mode() && RAW_MODE_ENABLED.load(Ordering::SeqCst) {
        println!("{}", crossterm::cursor::Show {});
        println!("{}", crossterm::cursor::DisableBlinking {});
        debug!("cursor enabled");
    }
}

pub fn set_title(name: &str) -> anyhow::Result<()> {
    use std::io::Write;

    if is_raw_mode() {
        execute!(std::io::stdout(), crossterm::terminal::SetTitle(name))?;
        info!("set window title to: {}", name);
    }

    Ok(())
}

pub struct TerminalEchoService {
    _input: Lifeline,
    _output: Lifeline,
}

impl Service for TerminalEchoService {
    type Bus = TerminalBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &TerminalBus) -> anyhow::Result<Self> {
        enable_raw_mode();

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
        disable_raw_mode();
    }
}

async fn forward_stdin(
    mut tx: impl Sender<TerminalSend>,
    mut tx_shutdown: impl Sender<TerminalShutdown>,
) -> anyhow::Result<()> {
    info!("listening for stdin");
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
            // write a newline.
            // this prevents a situation like this:
            // $ child terminal <ctrl-W> $ parent terminal
            tokio::io::stdout().write("\r\n".as_bytes()).await?;
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

    Ok(())
}
