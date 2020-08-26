use crate::prelude::*;
use crate::{
    message::{
        tab::TabRecv,
        tab_manager::{TabManagerRecv, TabManagerSend},
    },
    state::{
        assignment::{assignment, Retraction},
        tab::TabsState,
    },
};
use anyhow::Context;

use mpsc::error::TryRecvError;
use std::{
    collections::{HashMap, VecDeque},
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use tab_api::{
    launch::launch_pty,
    tab::{TabId, TabMetadata},
};
use tokio::{sync::mpsc, time};

pub struct TabManagerService {
    _recv: Lifeline,
    _retractions: Lifeline,
}

static TAB_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);
// working on a bug here where all the ptys disconnect, and TabRecv goes dead.
impl Service for TabManagerService {
    type Bus = ListenerBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut tx_tabs = bus.tx::<TabRecv>()?;
        let mut rx_retractions = bus.rx::<Retraction<TabMetadata>>()?.into_inner();
        let _retractions = {
            Self::try_task("retractions", async move {
                let mut retractions: VecDeque<Retraction<TabMetadata>> = VecDeque::new();
                let mut terminate = false;
                'monitor: loop {
                    'recv: loop {
                        match rx_retractions.try_recv() {
                            Ok(ret) => retractions.push_back(ret),
                            Err(TryRecvError::Empty) => {
                                break 'recv;
                            }
                            Err(TryRecvError::Closed) => {
                                terminate = true;
                                break 'recv;
                            }
                        }
                    }

                    let mut new_retractions = VecDeque::new();
                    while let Some(retraction) = retractions.pop_back() {
                        if retraction.is_taken() {
                            continue;
                        }

                        if let Some(metadata) =
                            retraction.retract_if_expired(Duration::from_millis(100))
                        {
                            debug!("regenerating assignment for tab {:?}", metadata.id);

                            let (ret, assign) = assignment(metadata);
                            let message = TabRecv::Assign(assign);
                            tx_tabs.send(message).await.ok();
                            new_retractions.push_back(ret);
                        } else {
                            new_retractions.push_back(retraction);
                        }
                    }
                    retractions.append(&mut new_retractions);
                    if terminate && retractions.is_empty() {
                        break 'monitor;
                    }

                    for _ in 0..retractions.len() {
                        // launches a new PTY process, using the current executible.
                        launch_pty()?;
                    }

                    time::delay_for(Duration::from_millis(100)).await;
                }

                Ok(())
            })
        };

        let _recv = {
            let mut rx = bus.rx::<TabManagerRecv>()?;

            let mut tx = bus.tx::<TabManagerSend>()?;
            let mut tx_tabs = bus.tx::<TabRecv>()?;
            let mut tx_tab_retraction = bus.tx::<Retraction<TabMetadata>>()?;
            let mut tx_tabs_state = bus.tx::<TabsState>()?;

            let mut tabs: HashMap<TabId, TabMetadata> = HashMap::new();

            Self::try_task("recv", async move {
                'msg: while let Some(msg) = rx.recv().await {
                    match msg {
                        TabManagerRecv::CreateTab(create) => {
                            for tab in tabs.values() {
                                if tab.name == create.name {
                                    debug!("got already created tab: {}", tab.name);
                                    continue 'msg;
                                }
                            }

                            debug!("recieved request to create tab {}", &create.name);
                            let id = TAB_ID_COUNTER.fetch_add(1, Ordering::SeqCst) as u16;
                            let tab_id = TabId(id);
                            let tab_metadata = TabMetadata::create(tab_id, create);

                            let (ret, assign) = assignment(tab_metadata.clone());
                            let message = TabRecv::Assign(assign);
                            tx_tabs.send(message).await.ok();
                            tx_tab_retraction
                                .send(ret)
                                .await
                                .context("tx_tab_retraction send message")?;

                            tabs.insert(tab_id, tab_metadata);
                            tx_tabs_state.send(TabsState::new(&tabs)).await?;
                        }
                        TabManagerRecv::CloseNamedTab(name) => {
                            let close_tab = tabs.values().find(|t| t.name == name);
                            if let Some(tab) = close_tab {
                                Self::close_tab(
                                    tab.id,
                                    &mut tabs,
                                    &mut tx,
                                    &mut tx_tabs,
                                    &mut tx_tabs_state,
                                )
                                .await?;
                            }
                        }
                        TabManagerRecv::CloseTab(close) => {
                            Self::close_tab(
                                close,
                                &mut tabs,
                                &mut tx,
                                &mut tx_tabs,
                                &mut tx_tabs_state,
                            )
                            .await?;
                        }
                    }
                }
                Ok(())
            })
        };

        Ok(Self {
            _retractions,
            _recv,
        })
    }
}

impl TabManagerService {
    async fn close_tab(
        id: TabId,
        tabs: &mut HashMap<TabId, TabMetadata>,
        tx: &mut impl Sender<TabManagerSend>,
        tx_close: &mut impl Sender<TabRecv>,
        tx_tabs_state: &mut impl Sender<TabsState>,
    ) -> anyhow::Result<()> {
        tabs.remove(&id);

        tx.send(TabManagerSend::TabTerminated(id))
            .await
            .context("tx TabTerminated")?;
        tx_close.send(TabRecv::Terminate(id)).await.ok();
        tx_tabs_state
            .send(TabsState::new(&tabs))
            .await
            .context("tx_tabs_state TabsState")?;

        debug!("got tabs: {:?}", tabs);

        Ok(())
    }
}
