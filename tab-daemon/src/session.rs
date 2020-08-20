use crate::{
    pty_process::PtyResponse,
    runtime::{DaemonRuntime, TabRuntime},
};
use futures::future::{AbortHandle, Abortable};
use futures::pin_mut;
use std::{collections::HashMap, sync::Arc};
use tab_api::{response::Response, tab::TabId};
use tokio::sync::mpsc::Sender;

pub struct DaemonSession {
    subscriptions: HashMap<TabId, AbortHandle>,
    runtime: Arc<DaemonRuntime>,
}

impl DaemonSession {
    pub fn new(runtime: Arc<DaemonRuntime>) -> Self {
        Self {
            subscriptions: HashMap::new(),
            runtime,
        }
    }

    pub async fn subscribe(&mut self, tab: &TabId, tx: Sender<Response>) -> anyhow::Result<()> {
        if self.subscriptions.contains_key(tab) {
            return Ok(());
        }

        let tab_runtime = self
            .runtime
            .get_tab(tab.0 as usize)
            .await
            .ok_or_else(|| anyhow::Error::msg(format!("no tab found with id: {:?}", tab)))?;

        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        let task = Self::spawn_subscription(tab.clone(), tab_runtime, tx);
        let future = Abortable::new(task, abort_registration);
        tokio::spawn(future);

        self.subscriptions.insert(tab.clone(), abort_handle);

        Ok(())
    }

    pub fn unsubscribe(&mut self, tab: TabId) {
        if let Some(subscription) = self.subscriptions.remove(&tab) {
            subscription.abort();
        }
    }

    pub fn runtime(&self) -> &DaemonRuntime {
        self.runtime.as_ref()
    }

    pub async fn spawn_subscription(
        tab: TabId,
        tab_runtime: Arc<TabRuntime>,
        mut tx: Sender<Response>,
    ) {
        let stream = tab_runtime.pty_sender().subscribe().await;
        pin_mut!(stream);
        while let Ok(response) = stream.recv().await {
            match response {
                PtyResponse::Output(chunk) => {
                    // TODO: error handling
                    let message = Response::Output(tab.clone(), chunk);
                    tx.send(message).await.expect("send failed");
                }
                PtyResponse::Terminated(_) => {
                    tx.send(Response::TabTerminated(tab))
                        .await
                        .expect("send termination failed");
                    break;
                }
            }
        }
    }
}
