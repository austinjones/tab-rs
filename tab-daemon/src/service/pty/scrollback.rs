use crate::{
    message::pty::{PtyRecv, PtySend},
    prelude::*,
    state::pty::PtyScrollback,
};

use std::{collections::VecDeque, sync::Arc};
use tab_api::chunk::OutputChunk;
use terminal_state::Tracker;
use tokio::sync::Mutex;

mod terminal_state;

/// Lines (LF-terminated) retained after pruning
const MAX_LINES: usize = 10_000;
/// Pruning only starts once this many extra lines have accumulated, so that it runs in batches
const PRUNE_SLACK: usize = 1_000;
/// Backstop for output with very long or no lines
const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_CHUNK_LEN: usize = 4096;

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
            "\x1b[6n".as_bytes().iter().copied().collect(),
            // replace ESC ] ** ; ? \x07, Operating System Command
            //   similarly, this sequence results in the terminal emulator echoing characters
            //   reference: https://www.xfree86.org/current/ctlseqs.html
            "\x1b]\x00\x00;?\x07".as_bytes().iter().copied().collect(),
            // replace ESC [ ** c, Send Device Attributes (Primary DA)
            //   similarly, this sequence results in the terminal emulator echoing characters
            //   reference: https://www.xfree86.org/current/ctlseqs.html
            "\x1b]\x00\x00c".as_bytes().iter().copied().collect(),
            // replace ESC [ = 0 c, Send Device Attributes (Tertiary DA)
            //   similarly, this sequence results in the terminal emulator echoing characters
            //   reference: https://www.xfree86.org/current/ctlseqs.html
            "\x1b]=0c".as_bytes().iter().copied().collect(),
            // replace ESC [ > ** ; ** ; 0 c, Send Device Attributes (Secondary DA)
            //   similarly, this sequence results in the terminal emulator echoing characters
            //   reference: https://www.xfree86.org/current/ctlseqs.html
            "\x1b]>\x00\x00;\x00\x00;0c"
                .as_bytes()
                .iter()
                .copied()
                .collect(),
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

/// The retained scrollback, and the terminal state needed to replay it from its first byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollbackSnapshot {
    /// Escape sequences that restore the terminal state at the start of `chunks`
    pub prelude: Vec<u8>,
    pub chunks: VecDeque<OutputChunk>,
}

#[derive(Debug)]
pub struct ScrollbackBuffer {
    size: usize,
    queue: VecDeque<OutputChunk>,
    /// Sees all output, to find line boundaries and track the current state (alt screen)
    tail: Tracker,
    /// Sees output up to the first retained byte, so its state is what a replay must restore
    head: Tracker,
    /// Stream indices just after each line feed in the retained output
    line_ends: VecDeque<usize>,
}

impl ScrollbackBuffer {
    pub fn new() -> Self {
        ScrollbackBuffer {
            size: 0,
            queue: VecDeque::new(),
            tail: Tracker::new(),
            head: Tracker::new(),
            line_ends: VecDeque::new(),
        }
    }

    pub fn push(&mut self, chunk: OutputChunk) {
        self.tail.feed(chunk.start(), &chunk.data);
        self.line_ends.extend(self.tail.take_lines());

        self.append(chunk);
        self.prune();
    }

    fn append(&mut self, mut chunk: OutputChunk) {
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

    fn prune(&mut self) {
        if let Some(index) = self.tail.take_hard_cut() {
            self.cut(index);
        }

        if self.line_ends.len() > MAX_LINES + PRUNE_SLACK {
            let target = self.line_ends[self.line_ends.len() - MAX_LINES - 1];

            // Keep a running alternate screen session intact, so that it can be redrawn.
            let target = match self.tail.alt_since() {
                Some(alt) => {
                    let before_alt = self.line_ends.partition_point(|&end| end <= alt);
                    before_alt
                        .checked_sub(1)
                        .map(|i| target.min(self.line_ends[i]))
                }
                None => Some(target),
            };

            if let Some(target) = target {
                self.cut(target);
            }
        }

        if self.size > MAX_BYTES {
            let start = self.queue.front().map_or(0, |chunk| chunk.start());
            let want = start + (self.size - (MAX_BYTES - MAX_BYTES / 8));
            let next_line = self.line_ends.partition_point(|&end| end < want);

            // Without a line boundary this may split an escape sequence, which only garbles the first line.
            let target = self.line_ends.get(next_line).copied().unwrap_or(want);
            self.cut(target);
        }
    }

    /// Discards all output before the stream index, which must be a line boundary or the end of an escape sequence.
    fn cut(&mut self, target: usize) {
        while let Some(front) = self.queue.front_mut() {
            if front.start() >= target {
                break;
            }

            if front.end() <= target {
                self.head.feed(front.start(), &front.data);
                self.size = self.size.saturating_sub(front.len());
                self.queue.pop_front();
            } else {
                let dropped = target - front.start();
                self.head.feed(front.start(), &front.data[..dropped]);
                self.size = self.size.saturating_sub(dropped);
                front.truncate_before(target);
                break;
            }
        }

        self.head.take_lines();
        self.head.take_hard_cut();

        while self.line_ends.front().map_or(false, |&end| end <= target) {
            self.line_ends.pop_front();
        }
    }

    pub fn snapshot(&self) -> ScrollbackSnapshot {
        ScrollbackSnapshot {
            prelude: self.head.state().prelude(),
            chunks: self.queue.clone(),
        }
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
        if sequence.is_empty() {
            return;
        }

        let mut index = 0;
        let mut seq_index = 0;

        while index <= buf.len() {
            if seq_index >= sequence.len() {
                debug!(
                    "a filtered ansi sequence was matched by the scrollback processor: {:?}",
                    sequence
                );
                debug!(
                    "the folowing data will be removed: {:?}",
                    &buf[index - sequence.len()..index]
                );

                buf.drain(index - sequence.len()..index);
                index -= sequence.len();
                seq_index = 0;
            }

            if index < buf.len()
                && (sequence[seq_index] == 0u8 || buf[index] == sequence[seq_index])
            {
                seq_index += 1;
            } else {
                seq_index = 0;
            }

            index += 1;
        }
    }
}

/// Tests of line-based pruning
#[cfg(test)]
mod buffer_tests {
    use super::{ScrollbackBuffer, MAX_BYTES, MAX_LINES, PRUNE_SLACK};
    use tab_api::chunk::OutputChunk;

    struct Fixture {
        buffer: ScrollbackBuffer,
        index: usize,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                buffer: ScrollbackBuffer::new(),
                index: 0,
            }
        }

        fn push(&mut self, data: &[u8]) {
            let chunk = OutputChunk {
                index: self.index,
                data: data.to_vec(),
            };
            self.index += data.len();
            self.buffer.push(chunk);
        }

        fn push_lines(&mut self, range: std::ops::Range<usize>) {
            for n in range {
                self.push(format!("line {}\n", n).as_bytes());
            }
        }

        fn retained(&self) -> String {
            let snapshot = self.buffer.snapshot();
            let data: Vec<u8> = snapshot
                .chunks
                .iter()
                .flat_map(|c| c.data.clone())
                .collect();
            String::from_utf8(data).unwrap()
        }

        fn prelude(&self) -> String {
            String::from_utf8(self.buffer.snapshot().prelude).unwrap()
        }
    }

    #[test]
    fn retains_everything_below_the_limit() {
        let mut fixture = Fixture::new();
        fixture.push_lines(0..MAX_LINES + PRUNE_SLACK);

        assert!(fixture.retained().starts_with("line 0\n"));
        assert_eq!("", fixture.prelude());
    }

    #[test]
    fn prunes_to_the_line_limit() {
        let mut fixture = Fixture::new();
        let total = MAX_LINES + PRUNE_SLACK + 1;
        fixture.push_lines(0..total);

        let retained = fixture.retained();
        assert_eq!(MAX_LINES, retained.lines().count());
        assert!(retained.starts_with(&format!("line {}\n", total - MAX_LINES)));
        assert!(retained.ends_with(&format!("line {}\n", total - 1)));
    }

    #[test]
    fn indices_stay_contiguous_after_pruning() {
        let mut fixture = Fixture::new();
        fixture.push_lines(0..MAX_LINES + PRUNE_SLACK + 500);

        let chunks = fixture.buffer.snapshot().chunks;
        assert!(chunks.front().unwrap().start() > 0);
        assert_eq!(fixture.index, chunks.back().unwrap().end());
        for pair in chunks.iter().collect::<Vec<_>>().windows(2) {
            assert_eq!(pair[0].end(), pair[1].start());
        }
    }

    #[test]
    fn prelude_restores_state_from_pruned_output() {
        let mut fixture = Fixture::new();
        fixture.push(b"\x1b[31m\x1b[?2004h");
        fixture.push_lines(0..MAX_LINES + PRUNE_SLACK + 1);

        assert_eq!("\x1b[0;38;5;1m\x1b[?2004h", fixture.prelude());
    }

    #[test]
    fn escape_sequences_split_across_chunks_are_not_cut() {
        let mut fixture = Fixture::new();
        fixture.push(b"\x1b[3");
        fixture.push(b"1m");
        fixture.push_lines(0..MAX_LINES + PRUNE_SLACK + 1);

        assert_eq!("\x1b[0;38;5;1m", fixture.prelude());
        assert!(fixture.retained().starts_with("line "));
    }

    #[test]
    fn alt_screen_session_is_never_pruned() {
        let mut fixture = Fixture::new();
        fixture.push(b"before\n\x1b[?1049h");
        fixture.push_lines(0..MAX_LINES + PRUNE_SLACK * 2);

        // output before the alt screen is pruned, but the session itself is kept from its first byte
        assert!(fixture.retained().starts_with("\x1b[?1049hline 0\n"));
        assert_eq!(
            MAX_LINES + PRUNE_SLACK * 2,
            fixture.retained().lines().count()
        );
        assert_eq!("", fixture.prelude());

        fixture.push(b"\x1b[?1049l");
        fixture.push_lines(0..10);

        assert!(!fixture.retained().starts_with("\x1b[?1049h"));
        assert_eq!(MAX_LINES + 10, fixture.retained().lines().count());
        // the cut falls inside the exited session, which the replay re-enters and then leaves again
        assert_eq!("\x1b[?1049h", fixture.prelude());
    }

    #[test]
    fn prelude_does_not_reenter_an_exited_alt_screen() {
        let mut fixture = Fixture::new();
        fixture.push(b"\x1b[?1049h");
        fixture.push_lines(0..10);
        fixture.push(b"\x1b[?1049l");

        fixture.push_lines(0..MAX_LINES + PRUNE_SLACK);
        assert_eq!("", fixture.prelude());
    }

    #[test]
    fn reset_discards_earlier_output() {
        let mut fixture = Fixture::new();
        fixture.push_lines(0..100);
        fixture.push(b"\x1b[1m\x1bc");
        fixture.push(b"after\n");

        assert_eq!("after\n", fixture.retained());
        assert_eq!("", fixture.prelude());
    }

    #[test]
    fn clear_scrollback_discards_earlier_output_but_keeps_state() {
        let mut fixture = Fixture::new();
        fixture.push(b"\x1b[1m");
        fixture.push_lines(0..100);
        fixture.push(b"\x1b[3J");
        fixture.push(b"after\n");

        assert_eq!("after\n", fixture.retained());
        assert_eq!("\x1b[0;1m", fixture.prelude());
    }

    #[test]
    fn byte_limit_bounds_output_without_line_feeds() {
        let mut fixture = Fixture::new();
        let chunk = vec![b'a'; 4096];
        for _ in 0..(MAX_BYTES / chunk.len()) + 100 {
            fixture.push(&chunk);
        }

        let retained = fixture.retained().len();
        assert!(retained <= MAX_BYTES);
        assert!(retained > MAX_BYTES / 2);
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

    #[test]
    fn test_separated_matches() {
        let filter = AnsiFilter::from_sequence(vec![2, 4]);

        let mut buf = vec![1, 2, 3, 4, 5];
        filter.filter(&mut buf);

        assert_eq!(buf, vec![1, 2, 3, 4, 5])
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

    #[test]
    fn bug_open_source() {
        let filter = ScrollbackManager::ansi_filter();

        let mut sequence = "open-source".as_bytes().into_iter().copied().collect();
        filter.filter(&mut sequence);

        assert_eq!("open-source".as_bytes(), sequence);
    }
}
