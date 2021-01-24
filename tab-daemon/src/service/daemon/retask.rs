use crate::{
    message::tab::{TabRecv, TabSend},
    prelude::*,
};

pub struct RetaskService {
    _fwd: Lifeline,
}

impl Service for RetaskService {
    type Bus = ListenerBus;

    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<TabRecv>()?;
        let mut tx = bus.tx::<TabSend>()?;

        let _fwd = Self::try_task("fwd", async move {
            debug!("retask service started");
            while let Some(msg) = rx.recv().await {
                if let TabRecv::Retask(from, to) = msg {
                    debug!("received retask request from {:?} to {:?}", from, to);
                    let msg = TabSend::Retask(from, to);
                    tx.send(msg).await?;
                }
            }

            Ok(())
        });
        Ok(Self { _fwd })
    }
}

#[cfg(test)]
mod tests {
    use super::RetaskService;
    use crate::{bus::ListenerBus, message::tab::TabRecv};
    use lifeline::{assert_completes, Bus, Receiver, Sender, Service};
    use tab_api::{client::RetaskTarget, tab::TabId};

    #[tokio::test]
    async fn echo() -> anyhow::Result<()> {
        let bus = ListenerBus::default();
        let _service = RetaskService::spawn(&bus);

        let mut tx = bus.tx::<TabRecv>()?;
        let mut rx = bus.rx::<TabRecv>()?;

        tx.send(TabRecv::Retask(TabId(0), RetaskTarget::Tab(TabId(1))))
            .await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(
                Some(TabRecv::Retask(TabId(0), RetaskTarget::Tab(TabId(1)))),
                msg
            );
        });

        Ok(())
    }

    #[tokio::test]
    async fn echo_disconnect() -> anyhow::Result<()> {
        let bus = ListenerBus::default();
        let _service = RetaskService::spawn(&bus);

        let mut tx = bus.tx::<TabRecv>()?;
        let mut rx = bus.rx::<TabRecv>()?;

        tx.send(TabRecv::Retask(TabId(0), RetaskTarget::Disconnect))
            .await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(
                Some(TabRecv::Retask(TabId(0), RetaskTarget::Disconnect)),
                msg
            );
        });

        Ok(())
    }

    #[tokio::test]
    async fn echo_select_interactive() -> anyhow::Result<()> {
        let bus = ListenerBus::default();
        let _service = RetaskService::spawn(&bus);

        let mut tx = bus.tx::<TabRecv>()?;
        let mut rx = bus.rx::<TabRecv>()?;

        tx.send(TabRecv::Retask(TabId(0), RetaskTarget::SelectInteractive))
            .await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(
                Some(TabRecv::Retask(TabId(0), RetaskTarget::SelectInteractive)),
                msg
            );
        });

        Ok(())
    }
}
