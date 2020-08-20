use super::state::{TabState, TabStateAvailable};
use futures::select;
use log::{debug, info};
use tab_api::tab::{TabId, TabMetadata};
use tab_service::{Lifeline, Service};
use tokio::{
    stream::StreamExt,
    sync::{broadcast, mpsc, watch},
};

#[derive(Clone, Debug)]
pub enum TabStateSelect {
    None,
    Selected(String),
}

impl Default for TabStateSelect {
    fn default() -> Self {
        Self::None
    }
}

pub struct TabStateService {}

pub struct TabStateRx {
    pub tab: watch::Receiver<TabStateSelect>,
    pub tab_metadata: broadcast::Receiver<TabMetadata>,
}

enum Event {
    Select(TabStateSelect),
    Metadata(TabMetadata),
}

impl Service for TabStateService {
    type Rx = TabStateRx;
    type Tx = watch::Sender<TabState>;
    type Lifeline = Lifeline;

    fn spawn(mut rx: Self::Rx, tx: Self::Tx) -> Self::Lifeline {
        Self::task("run", async move {
            let mut state = TabState::None;

            let mut events = {
                let tabs = rx.tab.map(|elem| Event::Select(elem));
                let tab_metadatas = rx.tab_metadata.map(|elem| Event::Metadata(elem.unwrap()));
                tabs.merge(tab_metadatas)
            };

            while let Some(event) = events.next().await {
                match event {
                    Event::Select(select) => match select {
                        TabStateSelect::None => {}
                        TabStateSelect::Selected(name) => {
                            let name = name.as_str();

                            if state.is_selected_name(name) || state.is_awaiting(name) {
                                continue;
                            }

                            state = TabState::Awaiting(name.to_string());
                            tx.broadcast(state.clone()).expect("tab state broadcast");
                        }
                    },
                    Event::Metadata(metadata) => {
                        if state.is_awaiting(metadata.name.as_str()) {
                            state = TabState::Selected(TabId(metadata.id), metadata.name);
                            tx.broadcast(state.clone()).expect("tab state broadcast");
                        }
                    }
                }
            }
        })
    }
}

// impl TabStateService {
//     pub fn find_tab<'a>(
//         tabs: &'a TabStateAvailable,
//         awaiting: &TabStateSelect,
//     ) -> Option<&'a TabMetadata> {
//         let tabs = &tabs.0;
//         let awaiting = &awaiting.0;

//         if !awaiting.is_some() {
//             return None;
//         }

//         let awaiting = awaiting.as_ref().unwrap();

//         tabs.iter().find(|metadata| &metadata.name == awaiting)
//     }
// }
