use super::{process::PtyProcess, receiver::PtyReceiver};
use crate::message::pty::{PtyRequest, PtyResponse};
use crate::prelude::*;
use async_trait::async_trait;
use lifeline::error::SendError;
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
        mpsc,
    },
    time,
};

#[derive(Clone)]
pub struct PtySender {
    tx_request: tokio::sync::mpsc::Sender<PtyRequest>,
    tx_response: tokio::sync::broadcast::Sender<PtyResponse>,
}

// TODO: rewrite as a proper service
impl PtySender {
    pub(super) fn new(
        tx_request: tokio::sync::mpsc::Sender<PtyRequest>,
        tx_response: tokio::sync::broadcast::Sender<PtyResponse>,
    ) -> Self {
        Self {
            tx_request,
            tx_response,
        }
    }

    pub async fn send(
        &mut self,
        request: PtyRequest,
    ) -> Result<(), mpsc::error::SendError<PtyRequest>> {
        self.tx_request.send(request).await
    }

    pub async fn subscribe(&self) -> PtyReceiver {
        PtyReceiver::new(self.tx_response.subscribe()).await
    }
}

#[async_trait]
impl crate::Sender<PtyRequest> for PtySender {
    async fn send(&mut self, value: PtyRequest) -> Result<(), SendError<PtyRequest>> {
        self.tx_request
            .send(value)
            .await
            .map_err(|err| SendError::Return(err.0))
    }
}
