use futures::Stream;
use log::info;
use std::{collections::VecDeque, process::Stdio, sync::Arc};
use tab_api::{
    chunk::{Chunk, ChunkType},
    tab::{CreateTabMetadata, TabId, TabMetadata},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command},
    stream,
    sync::{
        broadcast::{Receiver, Sender},
        RwLock,
    },
};

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
            metadata,
            process: TabProcess::new().await?,
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
    child: Child,
    tx: Sender<Chunk>,
    scrollback: Arc<RwLock<ScrollbackBuffer>>,
}

impl TabProcess {
    pub async fn new() -> anyhow::Result<TabProcess> {
        let mut child = Command::new("fish")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let scrollback: ArcLockScrollbackBuffer = Arc::new(RwLock::new(ScrollbackBuffer::new()));

        let (tx, rx) = tokio::sync::broadcast::channel(OUTPUT_CHANNEL_SIZE);
        let (tx_stdin, rx_stdin) = tokio::sync::mpsc::channel(STDIN_CHANNEL_SIZE);
        // scrollback writer
        tokio::spawn(Self::write_scrollback(scrollback.clone(), rx));

        // stdout reader
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(Self::read_channel(stdout, ChunkType::Stdout, tx.clone()));
        }

        // stderr reader
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(Self::read_channel(stderr, ChunkType::Stdout, tx.clone()));
        }

        if let Some(stdin) = child.stdin.take() {
            tokio::spawn(Self::write_stdin(stdin, rx_stdin));
        }

        Ok(TabProcess {
            child,
            tx,
            scrollback,
        })
    }

    pub async fn read(&self) -> impl Stream<Item = Chunk> {
        use async_stream::stream;

        let scrollback = self.scrollback.clone();
        let mut subscription = self.tx.subscribe();
        stream! {
            let mut accept_index = 0usize;

            {
                let scrollback = scrollback.read().await;
                for chunk in scrollback.chunks() {
                    accept_index = chunk.index + 1;
                    yield chunk.clone();
                }
            }

            for chunk in subscription.recv().await {
                if chunk.index >= accept_index {
                    accept_index = chunk.index + 1;
                    yield chunk;
                }
            }
        }
    }

    async fn read_channel(
        mut channel: impl AsyncReadExt + Unpin,
        channel_type: ChunkType,
        tx: Sender<Chunk>,
    ) {
        let mut index = 0usize;

        let mut buffer = vec![0u8; CHUNK_LEN];
        while let Ok(read) = channel.read(buffer.as_mut_slice()).await {
            if read == 0 {
                continue;
            }

            let mut buf = vec![0; read];
            buf.copy_from_slice(&buffer[0..read]);

            let chunk = Chunk {
                index,
                channel: channel_type.clone(),
                data: buf,
            };

            // TODO: deal with error handling
            tx.send(chunk).expect("Failed to send chunk");
            index += 1;
        }
    }

    async fn write_stdin(
        mut stdin: impl AsyncWriteExt + Unpin,
        mut rx: tokio::sync::mpsc::Receiver<Chunk>,
    ) {
        while let Some(mut chunk) = rx.recv().await {
            // TODO: deal with error handling
            stdin
                .write(chunk.data.as_mut_slice())
                .await
                .expect("Stdin write failed");
        }
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
        if let Some(back_len) = self.queue.back().map(Chunk::len) {
            if self.size - back_len + chunk.len() > self.min_capacity {
                self.queue.pop_back();
            }
        }

        // If we get several small buffers of the same channel, concat them.
        // This saves a lot of overhead for chunk id / channel storage over the websocket.
        // It does cause the client to 'miss' chunks, but that is part of the API contract.
        if let Some(front) = self.queue.front_mut() {
            if front.channel == chunk.channel && front.len() + chunk.len() < MAX_CHUNK_LEN {
                front.data.append(&mut chunk.data);
                return;
            }
        }

        self.queue.push_front(chunk)
    }

    pub fn chunks(&self) -> impl Iterator<Item = &Chunk> {
        self.queue.iter()
    }
}
