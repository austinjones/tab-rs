use crate::{
    message::pty::{PtyRecv, PtySend},
    prelude::*,
    state::pty::PtyScrollback,
};

use std::{collections::VecDeque, sync::Arc};
use tab_api::chunk::OutputChunk;
use tokio::sync::Mutex;

// 128MB memory limit
static MIN_CAPACITY: usize = 134217728;
static MAX_CHUNK_LEN: usize = 4096;

/// Spawns with a pty connection, and maintains a scrollback buffer.  Provides scrollback for tab-command clients
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
            let mut tx = bus.tx::<PtySend>()?;
            let serve_scrollback = buffer.clone();

            Self::try_task("serve", async move {
                while let Some(msg) = rx.recv().await {
                    if let PtyRecv::Scrollback = msg {
                        let scrollback = serve_scrollback.handle();
                        let response = PtySend::Scrollback(scrollback);
                        tx.send(response).await?;
                    }
                }

                Ok(())
            })
        };

        let _update = {
            let mut rx = bus.rx::<PtySend>()?;

            Self::try_task("serve", async move {
                while let Some(msg) = rx.recv().await {
                    if let PtySend::Output(output) = msg {
                        buffer.push(output).await;
                    }
                }

                Ok(())
            })
        };

        Ok(Self { _serve, _update })
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollbackBuffer {
    size: usize,
    pub(super) queue: VecDeque<OutputChunk>,
}

impl ScrollbackBuffer {
    pub fn new() -> Self {
        ScrollbackBuffer {
            size: 0,
            queue: VecDeque::new(),
        }
    }

    pub fn push(&mut self, mut chunk: OutputChunk) {
        // replace ESC [ 6n, Device Status Report
        // tbis sequence is echoed as keyboard characters,
        // and the tab session may not be running the same application as it was before
        replace_slice(
            chunk.data.as_mut_slice(),
            &['\x1b' as u8, '[' as u8, '6' as u8, 'n' as u8],
            &[],
        );

        if let Some(front_len) = self.queue.front().map(OutputChunk::len) {
            if self.size - front_len + chunk.len() > MIN_CAPACITY {
                self.size -= front_len;
                self.queue.pop_front();
            }
        }

        // If we get several small buffers, concat them.
        // This saves a lot of overhead for chunk id / channel storage over the websocket.
        // It does cause the client to 'miss' chunks, but that is part of the API contract.
        if let Some(back) = self.queue.back_mut() {
            if back.len() + chunk.len() < MAX_CHUNK_LEN {
                self.size += chunk.len();

                debug!(
                    "scrollback appending stdout chunk {}..{} to existing chunk {}..{}, size {}",
                    chunk.start(),
                    chunk.end(),
                    back.start(),
                    back.end(),
                    self.size,
                );
                back.data.append(&mut chunk.data);

                return;
            }
        }

        debug!(
            "scrollback pushing new chunk {}..{}, size {}",
            chunk.start(),
            chunk.end(),
            self.size + chunk.len()
        );

        self.size += chunk.len();
        self.queue.push_back(chunk);
    }

    pub fn clone_queue(&self) -> VecDeque<OutputChunk> {
        self.queue.clone()
    }
}

fn replace_slice(buf: &mut [u8], from: &[u8], to: &[u8]) {
    for i in 0..=buf.len() - from.len() {
        if buf[i..].starts_with(from) {
            buf[i..(i + from.len())].clone_from_slice(to);
        }
    }
}
