use crate::{
    message::{
        cli::CliRecv,
        tab::{TabRecv, TabSend},
    },
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
