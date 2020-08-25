use crate::{
    message::pty::{PtyRecv, PtySend},
    prelude::*,
    state::pty::PtyScrollback,
};
use lifeline::Service;
use std::{collections::VecDeque, sync::Arc};
use tab_api::chunk::OutputChunk;
use tokio::{stream::StreamExt, sync::Mutex};

pub struct PtyScrollbackService {
    _serve: Lifeline,
    _update: Lifeline,
}

impl Service for PtyScrollbackService {
    type Bus = PtyBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let buffer = ScrollbackManager::new();

        let _serve = {
            let mut rx = bus.rx::<PtyRecv>()?;
            let tx = bus.tx::<PtySend>()?;
            let serve_scrollback = buffer.clone();

            Self::try_task("serve", async move {
                while let Some(msg) = rx.next().await {
                    if let Ok(PtyRecv::Scrollback) = msg {
                        let scrollback = serve_scrollback.handle();
                        let response = PtySend::Scrollback(scrollback);
                        tx.send(response);
                    }
                }

                Ok(())
            })
        };

        let _update = {
            let mut rx = bus.rx::<PtySend>()?;

            Self::try_task("serve", async move {
                while let Some(msg) = rx.next().await {
                    if let Ok(PtySend::Output(output)) = msg {
                        buffer.push(output).await;
                    }
                }

                Ok(())
            })
        };

        Ok(Self { _serve, _update })
    }
}

static MAX_CHUNK_LEN: usize = 4096;

#[derive(Debug, Clone)]
struct ScrollbackManager {
    arc: Arc<Mutex<ScrollbackBuffer>>,
}

impl ScrollbackManager {
    pub fn new() -> Self {
        Self {
            arc: Arc::new(Mutex::new(ScrollbackBuffer::new())),
        }
    }

    pub fn handle(&self) -> PtyScrollback {
        PtyScrollback::new(self.arc.clone())
    }

    pub async fn push(&self, output: OutputChunk) {
        let mut buffer = self.arc.lock().await;
        buffer.push(output);
    }
}

#[derive(Debug, Clone)]
pub struct ScrollbackBuffer {
    size: usize,
    min_capacity: usize,
    pub(super) queue: VecDeque<OutputChunk>,
}

impl ScrollbackBuffer {
    pub fn new() -> Self {
        ScrollbackBuffer {
            size: 0,
            min_capacity: 8192,
            queue: VecDeque::new(),
        }
    }

    pub fn push(&mut self, mut chunk: OutputChunk) {
        if let Some(front_len) = self.queue.front().map(OutputChunk::len) {
            if self.size > front_len + chunk.len()
                && self.size - front_len + chunk.len() > self.min_capacity
            {
                self.queue.pop_back();
            }
        }

        // If we get several small buffers, concat them.
        // This saves a lot of overhead for chunk id / channel storage over the websocket.
        // It does cause the client to 'miss' chunks, but that is part of the API contract.
        if let Some(back) = self.queue.back_mut() {
            if back.len() + chunk.len() < MAX_CHUNK_LEN {
                back.data.append(&mut chunk.data);
                back.index = chunk.index;
                return;
            }
        }

        self.queue.push_back(chunk)
    }

    pub fn clone_queue(&self) -> VecDeque<OutputChunk> {
        self.queue.clone()
    }
}
