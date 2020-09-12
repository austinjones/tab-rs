use crate::{message::daemon::DaemonShutdown, prelude::*};
use listener::ListenerService;

use tab_api::config::dotdir_path;
use time::Duration;
use tokio::time;

mod listener;
mod retask;
mod tab_assignment;
mod tab_manager;

/// The main service for a tab-daemon service.  Spawns websocket listeners, and manages shutdown.
pub struct DaemonService {
    _listener: ListenerService,
    _shutdown: TabdirShutdownService,
}

impl Service for DaemonService {
    type Bus = DaemonBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> anyhow::Result<Self> {
        let _listener = ListenerService::spawn(bus)?;
        let _shutdown = TabdirShutdownService::spawn(bus)?;
        Ok(DaemonService {
            _listener,
            _shutdown,
        })
    }
}

/// If the service's tabdir is removed, shut down the daemon.
pub struct TabdirShutdownService {
    _monitor: Lifeline,
}

impl Service for TabdirShutdownService {
    type Bus = DaemonBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut tx = bus.tx::<DaemonShutdown>()?;
        let _monitor = Self::try_task("monitor", async move {
            loop {
                let config_dir = dotdir_path()?;
                if !config_dir.is_dir() {
                    info!(
                        "Daemon shutdown triggered by removed runtime directory: {}",
                        config_dir.as_path().to_string_lossy()
                    );
                    tx.send(DaemonShutdown {}).await.ok();
                    break;
                }

                time::delay_for(Duration::from_secs(2)).await;
            }

            Ok(())
        });

        Ok(Self { _monitor })
    }
}
