use crate::{message::pty::MainShutdown, prelude::*};

use super::client::ClientService;
use lifeline::dyn_bus::DynBus;
use std::fs;
use tab_api::config::dotdir_path;
use tab_websocket::{
    bus::{WebsocketCarrier, WebsocketConnectionBus},
    resource::connection::WebsocketResource,
};
use time::Duration;
use tokio::time;

pub struct MainService {
    _pty: ClientService,
    _carrier: WebsocketCarrier,
    _shutdown: TabdirShutdownService,
}

impl Service for MainService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let websocket = bus.resource::<WebsocketResource>()?;
        let websocket_connection_bus = WebsocketConnectionBus::default();
        websocket_connection_bus.store_resource(websocket);

        let _carrier = websocket_connection_bus.carry_from(bus)?;

        debug!("Launching MainService");
        let _pty = ClientService::spawn(bus)?;
        let _shutdown = TabdirShutdownService::spawn(bus)?;

        Ok(Self {
            _pty,
            _carrier,
            _shutdown,
        })
    }
}

/// If the service's tabdir is removed, shut down the daemon.
pub struct TabdirShutdownService {
    _monitor: Lifeline,
}

impl Service for TabdirShutdownService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut tx = bus.tx::<MainShutdown>()?;
        let _monitor = Self::try_task("monitor", async move {
            loop {
                let config_dir = dotdir_path()?;
                if !config_dir.is_dir() {
                    info!(
                        "shutdown triggered - tabdir was removed: {}",
                        config_dir.as_path().to_string_lossy()
                    );

                    tx.send(MainShutdown {}).await.ok();
                    break;
                }

                time::delay_for(Duration::from_secs(2)).await;
            }

            Ok(())
        });

        Ok(Self { _monitor })
    }
}
