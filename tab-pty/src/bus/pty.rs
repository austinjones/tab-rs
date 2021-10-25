use crate::{
    message::pty::{MainShutdown, PtyOptions, PtyRequest, PtyResponse, PtyShutdown},
    prelude::*,
};
use postage::{broadcast, mpsc};
use tab_api::pty::{PtyWebsocketRequest, PtyWebsocketResponse};

lifeline_bus!(pub struct PtyBus);

impl Resource<PtyBus> for PtyOptions {}

impl Message<PtyBus> for PtyRequest {
    type Channel = broadcast::Sender<Self>;
}

impl Message<PtyBus> for PtyWebsocketRequest {
    type Channel = mpsc::Sender<Self>;
}

impl Message<PtyBus> for PtyWebsocketResponse {
    type Channel = mpsc::Sender<Self>;
}

impl Message<PtyBus> for PtyResponse {
    type Channel = mpsc::Sender<Self>;
}

impl Message<PtyBus> for PtyShutdown {
    type Channel = broadcast::Sender<Self>;
}

pub struct MainPtyCarrier {
    _forward_request: Lifeline,
    _reply_response: Lifeline,
    _reply_shutdown: Lifeline,
}

impl CarryFrom<MainBus> for PtyBus {
    type Lifeline = anyhow::Result<MainPtyCarrier>;

    fn carry_from(&self, from: &MainBus) -> Self::Lifeline {
        let _forward_request = {
            let mut rx = from.rx::<PtyWebsocketRequest>()?;
            let mut tx = self.tx::<PtyWebsocketRequest>()?;
            Self::try_task("forward_request", async move {
                while let Some(msg) = rx.recv().await {
                    tx.send(msg).await?;
                }

                Ok(())
            })
        };

        let _reply_response = {
            let mut rx = self.rx::<PtyWebsocketResponse>()?;
            let mut tx = from.tx::<PtyWebsocketResponse>()?;
            Self::try_task("reply_response", async move {
                while let Some(msg) = rx.recv().await {
                    tx.send(msg).await?;
                }

                Ok(())
            })
        };

        // for now, a pty shutdown kills the process.
        // eventually we could reuse the pty, but why not just end?
        let _reply_shutdown = {
            let mut rx = self.rx::<PtyShutdown>()?;
            let mut tx = from.tx::<MainShutdown>()?;
            Self::task("forward_shutdown", async move {
                let shutdown_msg = rx.recv().await;
                if shutdown_msg.is_some() {
                    tx.send(MainShutdown {}).await.ok();
                }
            })
        };

        Ok(MainPtyCarrier {
            _forward_request,
            _reply_response,
            _reply_shutdown,
        })
    }
}
