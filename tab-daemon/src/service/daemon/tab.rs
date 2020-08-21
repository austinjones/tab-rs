use crate::bus::DaemonBus;
use crate::{bus::TabBus, message::daemon::CloseTab};
use std::sync::atomic::{AtomicUsize, Ordering};
use tab_api::tab::TabId;
use tab_service::{Bus, Lifeline, Service};
pub struct TabService {
    pub id: TabId,
    _run: Lifeline,
}

const TAB_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

impl Service for TabService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let id = TAB_ID_COUNTER.fetch_add(1, Ordering::SeqCst);

        let shutdown = bus.rx::<CloseTab>()?;
        let _run = Self::try_task("run_tab", async { Ok(()) });

        Ok(Self {
            id: TabId(id as u16),
            _run,
        })
    }
}
