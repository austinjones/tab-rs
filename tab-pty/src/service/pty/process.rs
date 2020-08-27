use crate::message::pty::{PtyOptions, PtyRequest, PtyResponse};
use crate::prelude::*;
use std::{
    collections::HashMap,
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

static CHUNK_LEN: usize = 2048;
static OUTPUT_CHANNEL_SIZE: usize = 32;
static STDIN_CHANNEL_SIZE: usize = 32;

#[derive(Debug, Clone)]
pub struct PtyProcess {}

impl PtyProcess {
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
        mut stdin: AsyncPtyMasterWriteHalf,
        mut rx: tokio::sync::mpsc::Receiver<PtyRequest>,
    ) {
        while let Some(request) = rx.recv().await {
            match request {
                PtyRequest::Resize(dimensions) => {
                    if let Err(e) = stdin.resize(dimensions).await {
                        error!("failed to resize pty: {:?}", e);
                    }

                    info!("resized to dimensions: {:?}", &dimensions);
                }
                PtyRequest::Input(chunk) => Self::write_stdin(&mut stdin, chunk).await,
                PtyRequest::Shutdown => {
                    info!("terminating pty");
                    stdin.shutdown();
                }
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
