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
    filter: AnsiFilter,
}

enum AnsiPattern {
    Char(char),
    Wildcard,
}

impl ScrollbackManager {
    pub fn new() -> Self {
        Self {
            arc: Arc::new(Mutex::new(ScrollbackBuffer::new())),
            filter: Self::ansi_filter(),
        }
    }

    /// Several ANSI escape sequences that should not be replayed   
    pub fn ansi_filter() -> AnsiFilter {
        AnsiFilter::new(vec![
            // replace ESC [ 6n, Device Status Report
            //   this sequence is echoed as keyboard characters,
            //   and the tab session may not be running the same application as it was before
            "\x1b[6n".as_bytes().into_iter().copied().collect(),
            // replace ESC ] ** ; ? \x07, Operating System Command
            //   similarly, this sequence results in the terminal emulator echoing characters
            //   reference: https://www.xfree86.org/current/ctlseqs.html
            "\x1b]\x00\x00;?\x07"
                .as_bytes()
                .into_iter()
                .copied()
                .collect(),
            // vec!['\x1b' as u8, '[' as u8, '6' as u8, 'n' as u8],
            // replace the BEL character, so the terminal doesn't re-play the bell
            vec![7u8],
        ])
    }

    pub fn handle(&self) -> PtyScrollback {
        PtyScrollback::new(self.arc.clone())
    }

    pub async fn push(&self, mut output: OutputChunk) {
        // replace ANSI escape sequences that should not be repeated when scrollback is re-played.
        self.filter.filter(&mut output.data);

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
#[derive(Debug, Clone)]
struct AnsiFilter {
    sequences: Vec<Vec<u8>>,
}

impl Default for AnsiFilter {
    fn default() -> Self {
        todo!()
    }
}

impl AnsiFilter {
    pub fn new<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = Vec<u8>>,
    {
        let sequences: Vec<Vec<u8>> = iter.into_iter().collect();
        Self { sequences }
    }

    #[cfg(test)]
    pub fn from_sequence(vec: Vec<u8>) -> Self {
        Self {
            sequences: vec![vec],
        }
    }

    pub fn filter(&self, buf: &mut Vec<u8>) {
        for seq in &self.sequences {
            Self::filter_seq(seq.as_slice(), buf);
        }
    }

    fn filter_seq(sequence: &[u8], buf: &mut Vec<u8>) {
        if sequence.len() == 0 {
            return;
        }

        let mut index = 0;
        let mut seq_index = 0;

        while index <= buf.len() {
            if seq_index >= sequence.len() {
                buf.drain(index - sequence.len()..index);
                index -= sequence.len();
                seq_index = 0;
            }

            if index < buf.len()
                && (sequence[seq_index] == 0u8 || buf[index] == sequence[seq_index])
            {
                seq_index += 1;
            }

            index += 1;
        }
    }
}

/// General tests of the ANSI filter utility
#[cfg(test)]
mod tests {
    use super::AnsiFilter;

    #[test]
    fn test_replace() {
        let filter = AnsiFilter::from_sequence(vec![2, 3]);

        let mut buf = vec![1, 2, 3, 4];
        filter.filter(&mut buf);

        assert_eq!(buf, vec![1, 4])
    }

    #[test]
    fn test_replace_first() {
        let mut buf = vec![1, 2, 3, 4];

        let filter = AnsiFilter::from_sequence(vec![1, 2]);
        filter.filter(&mut buf);

        assert_eq!(buf, vec![3, 4])
    }

    #[test]
    fn test_replace_last() {
        let mut buf = vec![1, 2, 3, 4];
        let filter = AnsiFilter::from_sequence(vec![4]);
        filter.filter(&mut buf);
        assert_eq!(buf, vec![1, 2, 3])
    }

    #[test]
    fn test_wildcard() {
        let filter = AnsiFilter::from_sequence(vec![2, 0]);

        let mut buf = vec![1, 2, 3, 4];
        filter.filter(&mut buf);

        assert_eq!(buf, vec![1, 4])
    }
}

/// Specific sequences that tab must remove from scrollback buffers
#[cfg(test)]
mod sequence_tests {
    use super::ScrollbackManager;

    #[test]
    fn device_status_report() {
        let filter = ScrollbackManager::ansi_filter();

        let mut sequence = "start-\x1b[6n-end"
            .as_bytes()
            .into_iter()
            .copied()
            .collect();
        filter.filter(&mut sequence);

        assert_eq!("start--end".as_bytes(), sequence);
    }

    #[test]
    fn operating_system_command() {
        let filter = ScrollbackManager::ansi_filter();

        let mut sequence = "start-\x1b]10;?\x07-end"
            .as_bytes()
            .into_iter()
            .copied()
            .collect();
        filter.filter(&mut sequence);

        assert_eq!("start--end".as_bytes(), sequence);
    }
}
