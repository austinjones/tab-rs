use crate::message::pty::{PtyOptions, PtyOutputBarrier, PtyRequest, PtyResponse, PtyShutdown};
use crate::prelude::*;

use lifeline::{barrier::*, Receiver, Sender};
use std::{
    process::{Command, Stdio},
    time::Duration,
};
use tab_api::{
    chunk::{InputChunk, OutputChunk},
    env::forward_env_std,
};
use tab_pty_process::CommandExt;
use tab_pty_process::{
    AsyncPtyMaster, AsyncPtyMasterReadHalf, AsyncPtyMasterWriteHalf, Child, PtyMaster,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    time,
};

static CHUNK_LEN: usize = 4096;
static OUTPUT_CHANNEL_SIZE: usize = 32;
static STDIN_CHANNEL_SIZE: usize = 256;

// mod process;
// mod receiver;
// mod sender;

/// Handles direct I/O interactions with the pty OS resource.
/// Handles shell-specific interactions (bash/fish/zsh).
pub struct PtyService {
    _run: Lifeline,
}

impl Service for PtyService {
    type Bus = PtyBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        bus.capacity::<PtyRequest>(STDIN_CHANNEL_SIZE)?;
        bus.capacity::<PtyResponse>(OUTPUT_CHANNEL_SIZE)?;

        let options = bus.resource::<PtyOptions>()?;
        let rx_request = bus.rx::<PtyRequest>()?;
        let rx_shutdown = bus.rx::<PtyShutdown>()?;
        let tx_response = bus.tx::<PtyResponse>()?;

        let _run = Self::try_task(
            "run",
            Self::run(options, rx_request, rx_shutdown, tx_response),
        );

        Ok(Self { _run })
    }
}

impl PtyService {
    async fn run(
        options: PtyOptions,
        rx_request: impl Receiver<PtyRequest> + Send + 'static,
        mut rx_shutdown: impl Receiver<PtyShutdown>,
        tx_response: impl Sender<PtyResponse> + Clone + Send + 'static,
    ) -> anyhow::Result<()> {
        let (child, read, write) = Self::create_pty(options).await?;
        let (tx_barrier, rx_barrier) = barrier();

        // stdout reader
        let _output = Self::task(
            "output",
            Self::read_output(read, tx_response.clone(), tx_barrier),
        );

        let _input = Self::task("input", Self::write_input(write, rx_request));

        let mut tx_exit = tx_response.clone();
        let _exit_code = Self::try_task("exit_code", async move {
            let exit_code = child.await?;
            rx_barrier.await;

            info!("Shell successfully terminated with exit code {}", exit_code);
            tx_exit.send(PtyResponse::Terminated).await?;

            Ok(())
        });

        rx_shutdown.recv().await;

        Ok(())
    }

    async fn create_pty(
        options: PtyOptions,
    ) -> anyhow::Result<(Child, AsyncPtyMasterReadHalf, AsyncPtyMasterWriteHalf)> {
        let pty = AsyncPtyMaster::open()?;

        let mut child = Command::new(options.command);
        child.current_dir(options.working_directory);
        child.args(options.args.as_slice());
        child.stderr(Stdio::inherit());

        forward_env_std(&mut child);

        for (k, v) in options.env {
            child.env(k, v);
        }

        let child = child.spawn_pty_async(&pty)?;

        pty.resize(options.dimensions)
            .await
            .expect("failed to resize pty");

        let (read, write) = pty.split();

        Ok((child, read, write))
    }

    async fn read_output(
        mut channel: impl AsyncReadExt + Unpin,
        mut tx: impl Sender<PtyResponse>,
        _barrier: Barrier<PtyOutputBarrier>,
    ) {
        let mut index = 0usize;
        let mut buffer = vec![0u8; CHUNK_LEN];
        while let Ok(read) = channel.read(buffer.as_mut_slice()).await {
            if read == 0 {
                break;
            }

            trace!("Read {} bytes", read);

            let mut buf = vec![0; read];
            buf.copy_from_slice(&buffer[0..read]);

            let chunk = OutputChunk { index, data: buf };
            let response = PtyResponse::Output(chunk);

            tx.send(response).await.ok();
            index += read;

            time::delay_for(Duration::from_micros(150)).await;
        }
    }

    async fn write_input(mut stdin: AsyncPtyMasterWriteHalf, mut rx: impl Receiver<PtyRequest>) {
        while let Some(request) = rx.recv().await {
            match request {
                PtyRequest::Resize(dimensions) => {
                    if let Err(e) = stdin.resize(dimensions).await {
                        error!("failed to resize pty: {:?}", e);
                    }

                    debug!("resized to dimensions: {:?}", &dimensions);
                }
                PtyRequest::Input(chunk) => Self::write_stdin(&mut stdin, chunk).await,
                PtyRequest::Shutdown => {
                    debug!("terminating pty");
                    stdin.shutdown();
                }
            }
        }

        debug!("stdin loop terminated");
    }

    async fn write_stdin(mut stdin: impl AsyncWriteExt + Unpin, mut chunk: InputChunk) {
        debug!("writing stdin chunk: {:?}", &chunk);

        // TODO: deal with error handling
        stdin
            .write(chunk.data.as_mut_slice())
            .await
            .expect("Stdin write failed");

        stdin.flush().await.expect("stdin flush failed");
    }
}
