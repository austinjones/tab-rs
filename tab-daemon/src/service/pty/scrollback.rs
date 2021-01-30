use crate::{
    message::pty::{PtyRecv, PtySend},
    prelude::*,
    state::pty::PtyScrollback,
};

use std::sync::Arc;
use tab_api::chunk::OutputChunk;
use tokio::sync::Mutex;

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
                        let scrollback = serve_scrollback.render().await;
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
                    match msg {
                        PtySend::Started(metadata) => {
                            buffer.resize(metadata.dimensions).await;
                        }
                        PtySend::Output(output) => {
                            buffer.push(output).await;
                        }
                        PtySend::Resized(size) => {
                            buffer.resize(size).await;
                        }
                        _ => {}
                    }
                }

                Ok(())
            })
        };

        Ok(Self { _serve, _update })
    }
}

#[derive(Clone)]
struct ScrollbackManager {
    arc: Arc<Mutex<ScrollbackBuffer>>,
}

impl ScrollbackManager {
    pub fn new() -> Self {
        Self {
            arc: Arc::new(Mutex::new(ScrollbackBuffer::new())),
        }
    }

    // /// Several ANSI escape sequences that should not be replayed
    // pub fn ansi_filter() -> AnsiFilter {
    //     AnsiFilter::new(vec![
    //         // replace ESC [ 6n, Device Status Report
    //         //   this sequence is echoed as keyboard characters,
    //         //   and the tab session may not be running the same application as it was before
    //         "\x1b[6n".as_bytes().into_iter().copied().collect(),
    //         // replace ESC ] ** ; ? \x07, Operating System Command
    //         //   similarly, this sequence results in the terminal emulator echoing characters
    //         //   reference: https://www.xfree86.org/current/ctlseqs.html
    //         "\x1b]\x00\x00;?\x07"
    //             .as_bytes()
    //             .into_iter()
    //             .copied()
    //             .collect(),
    //         // replace ESC [ ** c, Send Device Attributes (Primary DA)
    //         //   similarly, this sequence results in the terminal emulator echoing characters
    //         //   reference: https://www.xfree86.org/current/ctlseqs.html
    //         "\x1b]\x00\x00c".as_bytes().into_iter().copied().collect(),
    //         // replace ESC [ = 0 c, Send Device Attributes (Tertiary DA)
    //         //   similarly, this sequence results in the terminal emulator echoing characters
    //         //   reference: https://www.xfree86.org/current/ctlseqs.html
    //         "\x1b]=0c".as_bytes().into_iter().copied().collect(),
    //         // replace ESC [ > ** ; ** ; 0 c, Send Device Attributes (Secondary DA)
    //         //   similarly, this sequence results in the terminal emulator echoing characters
    //         //   reference: https://www.xfree86.org/current/ctlseqs.html
    //         "\x1b]>\x00\x00;\x00\x00;0c"
    //             .as_bytes()
    //             .into_iter()
    //             .copied()
    //             .collect(),
    //     ])
    // }

    pub async fn resize(&self, size: (u16, u16)) {
        let mut buffer = self.arc.lock().await;

        buffer.resize(size);
    }

    pub async fn render(&self) -> PtyScrollback {
        let buffer = self.arc.lock().await;

        let index = buffer.index;
        let data = buffer.render();

        PtyScrollback::new(index, data)
    }

    pub async fn push(&self, output: OutputChunk) {
        // // replace ANSI escape sequences that should not be repeated when scrollback is re-played.
        // self.filter.filter(&mut output.data);

        let mut buffer = self.arc.lock().await;
        buffer.push(output);
    }
}

pub struct ScrollbackBuffer {
    index: usize,
    parser: vt100::Parser,
}

impl ScrollbackBuffer {
    pub fn new() -> Self {
        ScrollbackBuffer {
            index: 0,
            parser: vt100::Parser::new(1, 1, 1000),
        }
    }

    pub fn push(&mut self, mut chunk: OutputChunk) {
        debug!(
            "scrollback pushing new chunk {}..{}",
            chunk.start(),
            chunk.end()
        );

        self.index = chunk.end();
        self.parser.process(chunk.data.as_slice());
    }

    pub fn resize(&mut self, (cols, rows): (u16, u16)) {
        self.parser.set_size(rows, cols);
    }

    pub fn render(&self) -> Vec<u8> {
        let screen = self.parser.screen();

        let mut data = Vec::new();
        data.append(&mut screen.title_formatted());
        data.append(&mut screen.input_mode_formatted());
        data.append(&mut screen.all_contents_formatted());
        data
    }

    pub fn index(&self) -> usize {
        self.index
    }
}
