use crate::pty_process::{PtyOptions, PtyProcess, PtyReceiver, PtyResponse, PtySender};

use log::info;
use std::{
    collections::VecDeque,
    process::{Command, ExitStatus},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};
use tab_api::tab::{CreateTabMetadata, TabId, TabMetadata};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::RwLock,
};

pub struct DaemonRuntime {
    tabs: RwLock<Vec<Arc<TabRuntime>>>,
}

impl DaemonRuntime {
    pub fn new() -> Self {
        Self {
            tabs: RwLock::new(Vec::new()),
        }
    }

    pub async fn create_tab(
        &self,
        create: &CreateTabMetadata,
    ) -> anyhow::Result<(Arc<TabRuntime>, PtyReceiver)> {
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
        tabs.get(index)
            .map(|arc| arc.clone())
            .filter(|tab| tab.is_running())
    }

    pub async fn find_tab(&self, name: &str) -> Option<Arc<TabRuntime>> {
        let tabs = self.tabs.read().await;
        tabs.iter()
            .filter(|tab| tab.is_running())
            .find(|tab| tab.name() == name)
            .map(|arc| arc.clone())
    }
}

pub struct TabRuntime {
    metadata: TabMetadata,
    pty_sender: PtySender,
    is_running: Arc<AtomicBool>,
}

impl TabRuntime {
    pub async fn spawn(metadata: TabMetadata) -> anyhow::Result<(Self, PtyReceiver)> {
        let pty_options = PtyOptions {
            command: "bash".to_string(),
            dimensions: metadata.dimensions,
        };

        let (tx, rx) = PtyProcess::spawn(pty_options).await?;

        let is_running = Arc::new(AtomicBool::new(true));

        let mut rx_close = tx.subscribe().await;
        let is_running_close = is_running.clone();
        let id = metadata.id;
        tokio::task::spawn(async move {
            while let Ok(msg) = rx_close.recv().await {
                if let PtyResponse::Terminated(_) = msg {
                    break;
                }
            }

            info!("tab {} terminated", id);
            is_running_close.store(false, Ordering::SeqCst);
        });

        let runtime = Self {
            metadata,
            is_running,
            pty_sender: tx,
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

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }
}
