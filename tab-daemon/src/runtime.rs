use crate::pty_process::{PtyOptions, PtyProcess, PtyReceiver, PtySender};
use futures::Stream;
use log::info;
use std::{
    collections::VecDeque,
    process::{Command, ExitStatus},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use tab_api::{
    chunk::{},
    tab::{CreateTabMetadata, TabId, TabMetadata},
};
use tab_pty_process::CommandExt;
use tab_pty_process::{AsyncPtyMaster, PtyMaster};
use tokio::sync::broadcast::RecvError;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
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

    pub async fn create_tab(&self, create: &CreateTabMetadata) -> anyhow::Result<(Arc<TabRuntime>, PtyReceiver)> {
        let mut tabs = self.tabs.write().await;
        let id = tabs.len();
        let metadata = TabMetadata {
            id: id as u16,
            name: create.name.clone(),
            dimensions: create.dimensions.clone(),
        };

        let (tab, rx) = TabRuntime::spawn(metadata).await?;
        let tab = Arc::new(tab);

        tabs.push(tab.clone());

        Ok((tab, rx))
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
    pty_sender: PtySender
}

impl TabRuntime {
    pub async fn spawn(metadata: TabMetadata) -> anyhow::Result<(Self, PtyReceiver)> {
        let pty_options = PtyOptions {
            command: "bash".to_string(),
            dimensions: metadata.dimensions
        };

        let (tx, rx) = PtyProcess::spawn(pty_options).await?;
        
        let runtime = Self {
            metadata,
            pty_sender: tx
        };

        Ok((runtime, rx))
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

    pub fn pty_sender(&self) -> &PtySender {
        &self.pty_sender
    }
}
