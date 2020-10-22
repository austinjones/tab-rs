use std::time::Duration;

use tokio::time;

use crate::{message::main::MainRecv, message::main::MainShutdown, prelude::*};
pub struct MainGlobalShutdownService {
    _run: Lifeline,
}

impl Service for MainGlobalShutdownService {
    type Bus = MainBus;

    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<MainRecv>()?;

        let mut tx = bus.tx::<Request>()?;
        let mut tx_shutdown = bus.tx::<MainShutdown>()?;

        let _run = Self::try_task("run", async move {
            while let Some(msg) = rx.recv().await {
                if let MainRecv::GlobalShutdown = msg {
                    tx.send(Request::GlobalShutdown).await?;
                    time::delay_for(Duration::from_millis(10)).await;
                    tx_shutdown.send(MainShutdown(0)).await?;
                }
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}
