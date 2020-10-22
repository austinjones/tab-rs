use std::sync::atomic::{AtomicBool, Ordering};

use crate::message::terminal::{TerminalInput, TerminalOutput, TerminalShutdown};
use crate::{message::terminal::TerminalSend, prelude::*};
use anyhow::Context;
use tab_api::env::is_raw_mode;
use tokio::io::{AsyncReadExt, AsyncWriteExt, Stdout};

use super::echo_input::{Action, InputFilter, KeyBindings};

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

pub struct TerminalEchoService {
    _input: Lifeline,
    _output: Lifeline,
}

impl Service for TerminalEchoService {
    type Bus = TerminalBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &TerminalBus) -> anyhow::Result<Self> {
        enable_raw_mode();

        let rx = bus.rx::<TerminalOutput>()?;
        let _output = Self::try_task("stdout", print_stdout(rx));

        let tx = bus.tx::<TerminalInput>()?;
        let tx_terminal = bus.tx::<TerminalSend>()?;
        let tx_shutdown = bus.tx::<TerminalShutdown>()?;
        let _input = Self::try_task("stdin", forward_stdin(tx, tx_terminal, tx_shutdown));

        Ok(TerminalEchoService { _input, _output })
    }
}

impl Drop for TerminalEchoService {
    fn drop(&mut self) {
        disable_raw_mode();
    }
}

async fn forward_stdin(
    mut tx: impl Sender<TerminalInput>,
    mut tx_terminal: impl Sender<TerminalSend>,
    mut tx_shutdown: impl Sender<TerminalShutdown>,
) -> anyhow::Result<()> {
    info!("listening for stdin");
    let mut stdin = tokio::io::stdin();
    let mut buffer = vec![0u8; 512];
    let mut filter: InputFilter = KeyBindings::default().into();

    while let Ok(read) = stdin.read(buffer.as_mut_slice()).await {
        if read == 0 {
            continue;
        }

        let input = filter.input(&buffer[0..read]);

        let buf: Vec<u8> = input.data.into();

        trace!("stdin chunk of len {}", read);

        tx.send(TerminalInput::Stdin(buf))
            .await
            .context("tx TerminalSend::Stdin")?;

        if let Some(action) = input.action {
            match action {
                Action::Disconnect => {
                    tx_shutdown.send(TerminalShutdown {}).await?;
                }
                Action::SelectInteractive => {
                    tx_terminal.send(TerminalSend::FuzzyRequest).await?;
                }
            }

            break;
        }
    }

    Ok(())
}

async fn print_stdout(mut rx: impl Receiver<TerminalOutput>) -> anyhow::Result<()> {
    trace!("Waiting on messages...");

    let mut stdout = tokio::io::stdout();
    let mut error_printed = false;

    while let Some(message) = rx.recv().await {
        match message {
            TerminalOutput::Stdout(data) => {
                let result = write_stdout(&mut stdout, data).await;

                if let Err(e) = result {
                    if !error_printed {
                        error!("failed to print stdout: {}", e);
                        error_printed = true;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn write_stdout(stdout: &mut Stdout, data: Vec<u8>) -> anyhow::Result<()> {
    if data.len() == 0 {
        return Ok(());
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

    Ok(())
}
