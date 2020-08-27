use super::process::PtyProcess;
use crate::message::pty::PtyResponse;
use async_trait::async_trait;
use lifeline::Receiver;
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
        broadcast::{self, Sender},
        mpsc::{self, error::SendError},
    },
    time,
};

#[derive(Debug)]
pub struct PtyReceiver {
    receiver: broadcast::Receiver<PtyResponse>,
    accept_index: usize,
}

impl PtyReceiver {
    pub(super) async fn new(receiver: broadcast::Receiver<PtyResponse>) -> PtyReceiver {
        PtyReceiver {
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

#[async_trait]
impl Receiver<PtyResponse> for PtyReceiver {
    async fn recv(&mut self) -> Option<PtyResponse> {
        PtyReceiver::recv(self).await.ok()
    }
}
