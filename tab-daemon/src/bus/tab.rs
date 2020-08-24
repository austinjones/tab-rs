use crate::{
    message::tab::{TabRecv, TabSend},
    prelude::*,
};
use tokio::{
    stream::StreamExt,
    sync::{broadcast, mpsc},
};

lifeline_bus!(pub struct TabBus);

impl Message<TabBus> for TabSend {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TabBus> for TabRecv {
    type Channel = broadcast::Sender<Self>;
}

pub struct ListenerTabCarrier {
    pub(super) _send: Lifeline,
    pub(super) _recv: Lifeline,
}

impl FromCarrier<ListenerBus> for TabBus {
    type Lifeline = anyhow::Result<ListenerTabCarrier>;

    fn carry_from(&self, from: &ListenerBus) -> Self::Lifeline {
        let _send = {
            let mut rx = self.rx::<TabSend>()?;
            let tx = from.tx::<TabSend>()?;

            Self::try_task("send", async move {
                while let Some(msg) = rx.recv().await {
                    debug!("forwarding {:?}", msg);
                    tx.send(msg).map_err(into_msg)?;
                }

                Ok(())
            })
        };

        let _recv = {
            let mut rx = from.rx::<TabRecv>()?;
            let tx = self.tx::<TabRecv>()?;

            Self::try_task("recv", async move {
                while let Some(msg) = rx.next().await {
                    if let Ok(msg) = msg {
                        tx.send(msg).map_err(into_msg)?;
                    }
                }

                Ok(())
            })
        };

        Ok(ListenerTabCarrier { _send, _recv })
    }
}
