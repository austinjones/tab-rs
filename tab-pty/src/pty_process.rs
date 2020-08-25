use log::info;
use std::{
    process::{Command, ExitStatus},
    sync::Arc,
};
use tab_api::chunk::{InputChunk, OutputChunk};
use tab_pty_process::CommandExt;
use tab_pty_process::{
    AsyncPtyMaster, AsyncPtyMasterReadHalf, AsyncPtyMasterWriteHalf, Child, PtyMaster,
};
use time::Duration;
use tokio::sync::broadcast::RecvError;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{
        broadcast::{Receiver, Sender},
        mpsc::error::SendError,
    },
    time,
};

// ! TODO: move into tab-pty-process

static CHUNK_LEN: usize = 2048;
static MAX_CHUNK_LEN: usize = 2048;
static OUTPUT_CHANNEL_SIZE: usize = 32;
static STDIN_CHANNEL_SIZE: usize = 32;

#[derive(Debug, Clone)]
pub enum PtyRequest {
    Resize((u16, u16)),
    Input(InputChunk),
}

#[derive(Debug, Clone)]
pub enum PtyResponse {
    Output(OutputChunk),
    Terminated(ExitStatus),
}

pub struct PtyOptions {
    pub dimensions: (u16, u16),
    pub command: String,
}

#[derive(Clone)]
pub struct PtySender {
    pty: Arc<PtyProcess>,
    tx_request: tokio::sync::mpsc::Sender<PtyRequest>,
    tx_response: tokio::sync::broadcast::Sender<PtyResponse>,
}

// TODO: rewrite as a proper service
impl PtySender {
    pub(super) fn new(
        pty: Arc<PtyProcess>,
        tx_request: tokio::sync::mpsc::Sender<PtyRequest>,
        tx_response: tokio::sync::broadcast::Sender<PtyResponse>,
    ) -> Self {
        Self {
            pty,
            tx_request,
            tx_response,
        }
    }

    pub async fn send(&mut self, request: PtyRequest) -> Result<(), SendError<PtyRequest>> {
        self.tx_request.send(request).await
    }

    pub async fn scrollback(&self) -> PtyScrollback {
        PtyScrollback::new(self.pty.clone())
    }

    pub async fn subscribe(&self) -> PtyReceiver {
        PtyReceiver::new(self.pty.clone(), self.tx_response.subscribe()).await
    }
}

#[derive(Debug, Clone)]
pub struct PtyScrollback {
    pty: Arc<PtyProcess>,
}

impl PtyScrollback {
    pub(super) fn new(pty: Arc<PtyProcess>) -> Self {
        Self { pty }
    }
}

pub struct PtyReceiver {
    _pty: Arc<PtyProcess>,
    receiver: Receiver<PtyResponse>,
    accept_index: usize,
}

impl PtyReceiver {
    pub(super) async fn new(pty: Arc<PtyProcess>, receiver: Receiver<PtyResponse>) -> PtyReceiver {
        PtyReceiver {
            _pty: pty,
            receiver,
            accept_index: 0,
        }
    }

    pub async fn recv(&mut self) -> Result<PtyResponse, RecvError> {
        loop {
            let message = self.receiver.recv().await?;

            if let PtyResponse::Output(ref chunk) = message {
                if chunk.index < self.accept_index {
                    continue;
                }
            }

            return Ok(message);
        }
    }
}

#[derive(Debug, Clone)]
pub struct PtyProcess {}

impl PtyProcess {
    pub async fn spawn(options: PtyOptions) -> anyhow::Result<(PtySender, PtyReceiver)> {
        let (child, read, write) = Self::create_pty(options).await?;
        let process = Arc::new(PtyProcess::new());

        let (tx_response, _rx_response) =
            tokio::sync::broadcast::channel::<PtyResponse>(OUTPUT_CHANNEL_SIZE);
        let (tx_request, rx_request) = tokio::sync::mpsc::channel::<PtyRequest>(STDIN_CHANNEL_SIZE);

        // stdout reader
        tokio::spawn(Self::read_output(read, tx_response.clone()));
        tokio::spawn(Self::write_input(write, rx_request));

        let tx_exit = tx_response.clone();
        // TODO: convert to lifeline task?
        tokio::spawn(async move {
            // TODO: error handling
            let exit_code = child.await.expect("Failed to get exit status");
            tx_exit
                .send(PtyResponse::Terminated(exit_code))
                .expect("Failed to send termination message");
        });

        let sender = PtySender::new(process, tx_request, tx_response);
        let receiver = sender.subscribe().await;
        Ok((sender, receiver))
    }

    async fn create_pty(
        options: PtyOptions,
    ) -> anyhow::Result<(Child, AsyncPtyMasterReadHalf, AsyncPtyMasterWriteHalf)> {
        let pty = AsyncPtyMaster::open()?;

        let mut child = Command::new(options.command);
        let child = child.spawn_pty_async(&pty)?;

        pty.resize(options.dimensions)
            .await
            .expect("failed to resize pty");

        let (read, write) = pty.split();

        Ok((child, read, write))
    }

    fn new() -> PtyProcess {
        PtyProcess {}
    }

    async fn read_output(mut channel: impl AsyncReadExt + Unpin, tx: Sender<PtyResponse>) {
        let mut index = 0usize;
        let mut buffer = vec![0u8; CHUNK_LEN];
        while let Ok(read) = channel.read(buffer.as_mut_slice()).await {
            if read == 0 {
                continue;
            }

            info!("Read {} bytes", read);

            let mut buf = vec![0; read];
            buf.copy_from_slice(&buffer[0..read]);

            let chunk = OutputChunk { index, data: buf };
            let response = PtyResponse::Output(chunk);
            // TODO: deal with error handling
            tx.send(response).expect("Failed to send chunk");
            index += 1;

            // a very short delay allows things to batch up
            // without any buffering, the message rate can get very high
            time::delay_for(Duration::from_millis(5)).await;
        }
    }

    async fn write_input(
        mut stdin: impl AsyncWriteExt + Unpin,
        mut rx: tokio::sync::mpsc::Receiver<PtyRequest>,
    ) {
        while let Some(request) = rx.recv().await {
            match request {
                PtyRequest::Resize(_dimensions) => {}
                PtyRequest::Input(chunk) => Self::write_stdin(&mut stdin, chunk).await,
            }
        }

        info!("stdin loop terminated");
    }

    async fn write_stdin(mut stdin: impl AsyncWriteExt + Unpin, mut chunk: InputChunk) {
        info!("writing stdin chunk: {:?}", &chunk);

        // TODO: deal with error handling
        stdin
            .write(chunk.data.as_mut_slice())
            .await
            .expect("Stdin write failed");

        stdin.flush().await.expect("stdin flush failed");
    }
}
