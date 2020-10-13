use tokio::sync::{broadcast, mpsc, watch};

use crate::{
    message::fuzzy::FuzzyEvent, message::fuzzy::FuzzyRecv, message::fuzzy::FuzzySelection,
    message::fuzzy::FuzzyShutdown, message::main::MainShutdown,
    message::terminal::TerminalShutdown, prelude::*, state::fuzzy::FuzzyMatchState,
    state::fuzzy::FuzzyQueryState, state::fuzzy::FuzzySelectState,
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

impl Message<FuzzyBus> for Option<FuzzySelectState> {
    type Channel = watch::Sender<Self>;
}

impl Message<FuzzyBus> for FuzzyEvent {
    type Channel = broadcast::Sender<Self>;
}

impl Message<FuzzyBus> for FuzzySelection {
    type Channel = mpsc::Sender<Self>;
}

impl Message<FuzzyBus> for FuzzyShutdown {
    type Channel = mpsc::Sender<Self>;
}

pub struct TerminalFuzzyCarrier {
    _recv: Lifeline,
    _selection: Lifeline,
    _forward_shutdown: Lifeline,
}

impl CarryFrom<TerminalBus> for FuzzyBus {
    type Lifeline = anyhow::Result<TerminalFuzzyCarrier>;

    fn carry_from(&self, from: &TerminalBus) -> Self::Lifeline {
        let _recv = {
            let mut rx = from.rx::<FuzzyRecv>()?;
            let mut tx = self.tx::<FuzzyRecv>()?;

            Self::task("recv", async move {
                while let Some(msg) = rx.recv().await {
                    tx.send(msg).await.ok();
                }
            })
        };

        let _selection = {
            let mut rx = self.rx::<FuzzySelection>()?;
            let mut tx = from.tx::<FuzzySelection>()?;

            Self::task("recv", async move {
                while let Some(msg) = rx.recv().await {
                    tx.send(msg).await.ok();
                }
            })
        };

        let _forward_shutdown = {
            let mut rx = self.rx::<FuzzyShutdown>()?;
            let mut tx = from.tx::<TerminalShutdown>()?;

            Self::task("recv", async move {
                if let Some(_shutdown) = rx.recv().await {
                    tx.send(TerminalShutdown {}).await.ok();
                }
            })
        };

        Ok(TerminalFuzzyCarrier {
            _recv,
            _selection,
            _forward_shutdown,
        })
    }
}
