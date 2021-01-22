use crate::message::pty::{PtyOptions, PtyRequest, PtyResponse, PtyShutdown};
use crate::prelude::*;

use postage::barrier;
use std::{process::Stdio, time::Duration};
use tab_api::{
    chunk::{InputChunk, OutputChunk},
    env::forward_env,
};
use tab_pty_process::{
    unix::{UnixPtyMaster, UnixPtySystem, UnixPtyWrite},
    Child, Master, PtySystem, PtySystemInstance, PtySystemOptions, Size,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
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
        rx_request: impl Stream<Item = PtyRequest> + Unpin + Send + 'static,
        mut rx_shutdown: impl Stream<Item = PtyShutdown> + Unpin,
        tx_response: impl Sink<Item = PtyResponse> + Clone + Unpin + Send + 'static,
    ) -> anyhow::Result<()> {
        let system = Self::create_pty(options).await?;
        let (tx_barrier, mut rx_barrier) = barrier::channel();

        // stdout reader
        let _output = Self::task(
            "output",
            Self::read_output(system.read, tx_response.clone(), tx_barrier),
        );

        let _input = Self::task(
            "input",
            Self::write_input(system.master, system.write, rx_request),
        );

        let child = system.child;
        let mut tx_exit = tx_response.clone();
        let _exit_code = Self::try_task("exit_code", async move {
            let exit_code = child.wait().await?;
            rx_barrier.recv().await;

            info!("Shell successfully terminated with exit code {}", exit_code);
            tx_exit.send(PtyResponse::Terminated).await?;

            Ok(())
        });

        rx_shutdown.recv().await;

        Ok(())
    }

    async fn create_pty(options: PtyOptions) -> anyhow::Result<PtySystemInstance<UnixPtySystem>> {
        let mut child = Command::new(options.command);
        child.current_dir(options.working_directory);
        child.args(options.args.as_slice());
        child.stderr(Stdio::inherit());

        forward_env(&mut child);

        for (k, v) in options.env {
            child.env(k, v);
        }

        let system = UnixPtySystem::spawn(child, PtySystemOptions { raw_mode: false })?;

        let size = Size {
            rows: options.dimensions.0,
            cols: options.dimensions.1,
        };

        system
            .master
            .resize(size)
            .await
            .expect("failed to resize pty");

        Ok(system)
    }

    async fn read_output(
        mut channel: impl AsyncReadExt + Unpin,
        mut tx: impl Sink<Item = PtyResponse> + Unpin,
        _output_barrier: barrier::Sender,
    ) {
        let mut index = 0usize;
        let mut buffer = vec![0u8; CHUNK_LEN];
        while let Ok(read) = channel.read(buffer.as_mut_slice()).await {
            if read == 0 {
                debug!("Received {} bytes", read);
                break;
            }

            debug!("Received {} bytes", read);

            let mut buf = vec![0; read];
            buf.copy_from_slice(&buffer[0..read]);

            let chunk = OutputChunk { index, data: buf };
            let response = PtyResponse::Output(chunk);

            tx.send(response).await.ok();
            index += read;

            time::sleep(Duration::from_micros(150)).await;
        }
    }

    async fn write_input(
        master: UnixPtyMaster,
        mut stdin: UnixPtyWrite,
        mut rx: impl Stream<Item = PtyRequest> + Unpin,
    ) -> anyhow::Result<()> {
        while let Some(request) = rx.recv().await {
            match request {
                PtyRequest::Resize(dimensions) => {
                    let size = Size {
                        cols: dimensions.0,
                        rows: dimensions.1,
                    };

                    if let Err(e) = master.resize(size).await {
                        error!("failed to resize pty: {:?}", e);
                    }

                    debug!("resized to dimensions: {:?}", &dimensions);
                }
                PtyRequest::Input(chunk) => Self::write_stdin(&mut stdin, chunk).await,
                PtyRequest::Shutdown => {
                    debug!("terminating pty");
                    stdin.shutdown().await?;
                }
            }
        }

        debug!("stdin loop terminated");
        Ok(())
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
