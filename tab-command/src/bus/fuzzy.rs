use tokio::sync::{broadcast, mpsc, watch};

use crate::{
    message::fuzzy::FuzzyEvent, message::fuzzy::FuzzyRecv, message::fuzzy::FuzzyShutdown,
    message::main::MainShutdown, prelude::*, state::fuzzy::FuzzyMatchState,
    state::fuzzy::FuzzyQueryState,
};

lifeline_bus!(pub struct FuzzyBus);

impl Message<FuzzyBus> for FuzzyRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<FuzzyBus> for FuzzyQueryState {
    type Channel = watch::Sender<Self>;
}

impl Message<FuzzyBus> for FuzzyMatchState {
    type Channel = watch::Sender<Self>;
}

impl Message<FuzzyBus> for FuzzyEvent {
    type Channel = broadcast::Sender<Self>;
}

impl Message<FuzzyBus> for FuzzyShutdown {
    type Channel = mpsc::Sender<Self>;
}

pub struct TabFuzzyCarrier {
    _forward_recv: Lifeline,
}

impl CarryFrom<TabBus> for FuzzyBus {
    type Lifeline = anyhow::Result<TabFuzzyCarrier>;

    fn carry_from(&self, from: &TabBus) -> Self::Lifeline {
        let mut rx = from.rx::<FuzzyRecv>()?;
        let mut tx = self.tx::<FuzzyRecv>()?;

        let _forward_recv = Self::try_task("recv", async move {
            while let Some(msg) = rx.recv().await {
                tx.send(msg).await.ok();
            }

            Ok(())
        });

        Ok(TabFuzzyCarrier { _forward_recv })
    }
}

pub struct MainFuzzyCarrier {
    _forward_shutdown: Lifeline,
}

impl CarryFrom<MainBus> for FuzzyBus {
    type Lifeline = anyhow::Result<MainFuzzyCarrier>;

    fn carry_from(&self, from: &MainBus) -> Self::Lifeline {
        let mut rx = self.rx::<FuzzyShutdown>()?;
        let mut tx = from.tx::<MainShutdown>()?;

        let _forward_shutdown = Self::task("recv", async move {
            rx.recv().await;
            tx.send(MainShutdown {}).await.ok();
        });

        Ok(MainFuzzyCarrier { _forward_shutdown })
    }
}
