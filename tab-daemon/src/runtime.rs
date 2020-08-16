use futures::Stream;
use log::info;
use std::{
    collections::VecDeque,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tab_api::{
    chunk::{Chunk, ChunkType, StdinChunk},
    tab::{CreateTabMetadata, TabId, TabMetadata},
};
use tokio::sync::broadcast::RecvError;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStderr, ChildStdin, ChildStdout},
    stream,
    sync::{
        broadcast::{Receiver, Sender},
        RwLock,
    },
};
use tab_pty_process::CommandExt;
use tab_pty_process::{AsyncPtyFd, AsyncPtyMaster, PtyMaster};

static CHUNK_LEN: usize = 2048;
static MAX_CHUNK_LEN: usize = 2048;
static OUTPUT_CHANNEL_SIZE: usize = 32;
static STDIN_CHANNEL_SIZE: usize = 32;

pub struct DaemonRuntime {
    tabs: RwLock<Vec<Arc<TabRuntime>>>,
}

impl DaemonRuntime {
    pub fn new() -> Self {
        Self {
            tabs: RwLock::new(Vec::new()),
        }
    }

    pub async fn create_tab(&self, create: &CreateTabMetadata) -> anyhow::Result<Arc<TabRuntime>> {
        let mut tabs = self.tabs.write().await;
        let id = tabs.len();
        let metadata = TabMetadata {
            id: id as u16,
            name: create.name.clone(),
            dimensions: create.dimensions.clone(),
        };
        let tab_runtime = Arc::new(TabRuntime::new(metadata).await?);

        tabs.push(tab_runtime.clone());

        Ok(tab_runtime)
    }

    pub async fn get_tab(&self, index: usize) -> Option<Arc<TabRuntime>> {
        let tabs = self.tabs.read().await;
        tabs.get(index).map(|arc| arc.clone())
    }

    pub async fn find_tab(&self, name: &str) -> Option<Arc<TabRuntime>> {
        let tabs = self.tabs.read().await;
        tabs.iter()
            .find(|tab| tab.name() == name)
            .map(|arc| arc.clone())
    }
}

pub struct TabRuntime {
    metadata: TabMetadata,
    process: TabProcess,
}

impl TabRuntime {
    pub async fn new(metadata: TabMetadata) -> anyhow::Result<Self> {
        let runtime = Self {
            process: TabProcess::new(metadata.dimensions.clone()).await?,
            metadata,
        };

        Ok(runtime)
    }

    pub fn id(&self) -> TabId {
        TabId(self.metadata.id)
    }

    pub fn name(&self) -> &str {
        self.metadata.name.as_str()
    }

    pub fn metadata(&self) -> &TabMetadata {
        &self.metadata
    }

    pub fn process(&self) -> &TabProcess {
        &self.process
    }
}

pub struct TabProcess {
    child: tab_pty_process::Child,
    tx: Sender<Chunk>,
    tx_stdin: tokio::sync::mpsc::Sender<StdinChunk>,
    scrollback: Arc<RwLock<ScrollbackBuffer>>,
}

impl TabProcess {
    pub async fn new(dimensions: (u16, u16)) -> anyhow::Result<TabProcess> {
        // let mut child = Command::new("fish")
        //     .args(&["--interactive", "--debug=debug,proc-internal-proc"])
        let pty = AsyncPtyMaster::open()?;

        let mut child = Command::new("bash");
        let child = child.spawn_pty_async(&pty)?;

        pty.resize(dimensions).await.expect("failed to resize pty");

        let (read, write) = pty.split();

        let scrollback: ArcLockScrollbackBuffer = Arc::new(RwLock::new(ScrollbackBuffer::new()));

        let (tx, rx) = tokio::sync::broadcast::channel(OUTPUT_CHANNEL_SIZE);
        let (tx_stdin, rx_stdin) = tokio::sync::mpsc::channel(STDIN_CHANNEL_SIZE);
        // scrollback writer
        tokio::spawn(Self::write_scrollback(scrollback.clone(), rx));

        let write_index = Arc::new(AtomicUsize::new(0));
        // stdout reader
        tokio::spawn(Self::read_channel(
            write_index.clone(),
            read,
            ChunkType::Stdout,
            tx.clone(),
        ));
        tokio::spawn(Self::write_stdin(write, rx_stdin));

        Ok(TabProcess {
            child,
            tx,
            tx_stdin,
            scrollback,
        })
    }

    pub async fn read(&self) -> impl Stream<Item = Chunk> {
        use async_stream::stream;

        let mut subscription = self.tx.subscribe();
        let scrollback = self.scrollback.clone();
        stream! {
            let mut accept_index = 0usize;

            {
                let scrollback = scrollback.read().await;
                for chunk in scrollback.chunks() {
                    accept_index = chunk.index + 1;
                    info!("scrollback chunk: {:?}", chunk);
                    yield chunk.clone();
                }
            }

            info!("done with scrollback!");

            loop {
                let message = subscription.recv().await;
                match message {
                    Ok(chunk) => {
                        info!("recv chunk: {:?}", chunk);
                        if chunk.index >= accept_index {
                            accept_index = chunk.index + 1;
                            yield chunk;
                        } else {
                            info!("ignoring out-of-order chunk! {:?}", chunk);
                        }
                    },
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => { break; }
                }
            }

            info!("done with subscription")
        }
    }

    pub async fn write(&self, chunk: StdinChunk) -> anyhow::Result<()> {
        // todo: do better than cloning each time.  maybe keep a copy in the tab session?
        let mut tx = self.tx_stdin.clone();
        tx.send(chunk).await?;
        Ok(())
    }

    async fn read_channel(
        index: Arc<AtomicUsize>,
        mut channel: impl AsyncReadExt + Unpin,
        channel_type: ChunkType,
        tx: Sender<Chunk>,
    ) {
        let index = index.as_ref();
        let mut buffer = vec![0u8; CHUNK_LEN];
        while let Ok(read) = channel.read(buffer.as_mut_slice()).await {
            if read == 0 {
                continue;
            }

            info!("Read {} bytes from {:?}", read, channel_type);

            let mut buf = vec![0; read];
            buf.copy_from_slice(&buffer[0..read]);

            let chunk = Chunk {
                index: index.load(Ordering::SeqCst),
                channel: channel_type.clone(),
                data: buf,
            };

            // TODO: deal with error handling
            tx.send(chunk).expect("Failed to send chunk");
            index.fetch_add(1, Ordering::SeqCst);
        }
    }

    async fn write_stdin(
        mut stdin: impl AsyncWriteExt + Unpin,
        mut rx: tokio::sync::mpsc::Receiver<StdinChunk>,
    ) {
        //TODO: remove debugging statement
        stdin
            .write("echo from-rust\n".as_bytes())
            .await
            .expect("stdin write failed");

        while let Some(mut chunk) = rx.recv().await {
            info!("writing stdin chunk: {:?}", &chunk);

            // TODO: refactor this into a shared sender struct
            // tx.send(Chunk {
            //     index: index.load(Ordering::SeqCst),
            //     channel: ChunkType::Stdout,
            //     data: chunk.data.clone(),
            // })
            // .expect("stdin echo failed");
            // index.fetch_add(1, Ordering::SeqCst);

            // TODO: deal with error handling
            stdin
                .write(chunk.data.as_mut_slice())
                .await
                .expect("Stdin write failed");

            stdin.flush().await.expect("stdin flush failed");
        }

        info!("stdin loop terminated");
    }

    async fn write_scrollback(scrollback: ArcLockScrollbackBuffer, mut rx: Receiver<Chunk>) {
        while let Ok(chunk) = rx.recv().await {
            let mut lock = scrollback.write().await;

            lock.push(chunk);
        }
    }
}

type ArcLockScrollbackBuffer = Arc<RwLock<ScrollbackBuffer>>;
struct ScrollbackBuffer {
    size: usize,
    min_capacity: usize,
    queue: VecDeque<Chunk>,
}

impl ScrollbackBuffer {
    pub fn new() -> Self {
        ScrollbackBuffer {
            size: 0,
            min_capacity: 8192,
            queue: VecDeque::new(),
        }
    }

    pub fn push(&mut self, mut chunk: Chunk) {
        if let Some(front_len) = self.queue.front().map(Chunk::len) {
            if self.size > front_len + chunk.len()
                && self.size - front_len + chunk.len() > self.min_capacity
            {
                self.queue.pop_back();
            }
        }

        // If we get several small buffers of the same channel, concat them.
        // This saves a lot of overhead for chunk id / channel storage over the websocket.
        // It does cause the client to 'miss' chunks, but that is part of the API contract.
        if let Some(back) = self.queue.back_mut() {
            if back.channel == chunk.channel && back.len() + chunk.len() < MAX_CHUNK_LEN {
                back.data.append(&mut chunk.data);
                back.index = chunk.index;
                return;
            }
        }

        self.queue.push_back(chunk)
    }

    pub fn chunks(&self) -> impl Iterator<Item = &Chunk> {
        self.queue.iter()
    }
}
