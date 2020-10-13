use crate::{
    message::fuzzy::FuzzyRecv, message::fuzzy::FuzzySelection, message::terminal::TerminalRecv,
    message::terminal::TerminalSend, prelude::*,
};
use crate::{
    message::{
        main::MainShutdown,
        terminal::{TerminalInput, TerminalOutput, TerminalShutdown},
    },
    state::{
        tab::TabState,
        terminal::{TerminalMode, TerminalSizeState},
    },
};
use anyhow::Context;
use lifeline::prelude::*;
use tab_api::chunk::InputChunk;
use tokio::sync::{broadcast, mpsc, watch};

lifeline_bus!(pub struct TerminalBus);

impl Message<TerminalBus> for TerminalInput {
    type Channel = broadcast::Sender<Self>;
}

impl Message<TerminalBus> for TerminalOutput {
    type Channel = broadcast::Sender<Self>;
}

impl Message<TerminalBus> for TerminalSizeState {
    type Channel = watch::Sender<Self>;
}

impl Message<TerminalBus> for TerminalMode {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TerminalBus> for TerminalShutdown {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TerminalBus> for FuzzyRecv {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TerminalBus> for FuzzySelection {
    type Channel = mpsc::Sender<Self>;
}

/// Carries messages between the MainBus, and the TerminalBus.
///
/// Listens to MainRecv and sends TerminalMode,
/// forwards TerminalShutdown, and carries Input, Output, and Resize events.
pub struct MainTerminalCarrier {
    pub(super) _recv: Lifeline,
    pub(super) _fuzzy_send: Lifeline,
    pub(super) _forward_shutdown: Lifeline,
    pub(super) _echo_output: Lifeline,
    pub(super) _read_input: Lifeline,
}

impl CarryFrom<MainBus> for TerminalBus {
    type Lifeline = anyhow::Result<MainTerminalCarrier>;

    fn carry_from(&self, from: &MainBus) -> Self::Lifeline {
        let _recv = {
            let mut rx = from.rx::<TerminalRecv>()?;
            let mut tx_mode = self.tx::<TerminalMode>()?;
            let mut tx_fuzzy = self.tx::<FuzzyRecv>()?;

            Self::try_task("forward_mode", async move {
                while let Some(msg) = rx.recv().await {
                    match msg {
                        TerminalRecv::FuzzyTabs(tabs) => {
                            tx_fuzzy.send(FuzzyRecv { tabs }).await?;
                        }
                        TerminalRecv::Mode(mode) => {
                            tx_mode.send(mode).await?;
                        }
                    }
                }

                Ok(())
            })
        };

        let _fuzzy_send = {
            let mut rx = self.rx::<FuzzySelection>()?;
            let mut tx = from.tx::<TerminalSend>()?;

            Self::try_task("forward_mode", async move {
                while let Some(msg) = rx.recv().await {
                    tx.send(TerminalSend::FuzzySelection(msg.0)).await?;
                }

                Ok(())
            })
        };

        let _forward_shutdown = {
            let mut rx_shutdown = self.rx::<TerminalShutdown>()?;
            let mut tx_shutdown = from.tx::<MainShutdown>()?;

            Self::try_task("forward_shutdown", async move {
                if let Some(_shutdown) = rx_shutdown.recv().await {
                    tx_shutdown.send(MainShutdown {}).await?;
                }

                Ok(())
            })
        };

        let _echo_output = {
            let mut rx_response = from.rx::<Response>()?;
            let mut tx_output = self.tx::<TerminalOutput>()?;

            Self::try_task("main_recv", async move {
                while let Some(response) = rx_response.recv().await {
                    match response {
                        Response::Output(_id, stdout) => {
                            tx_output
                                .send(TerminalOutput::Stdout(stdout.data))
                                .await
                                .context("tx TerminalRecv::Stdout")?;
                        }
                        _ => {}
                    }
                }

                Ok(())
            })
        };

        let _read_input = {
            let rx_tab_state = from.rx::<TabState>()?.into_inner();
            let mut rx_terminal_input = self.rx::<TerminalInput>()?;
            let mut tx_request = from.tx::<Request>()?;

            Self::try_task("main_recv", async move {
                while let Some(msg) = rx_terminal_input.recv().await {
                    match msg {
                        TerminalInput::Stdin(data) => {
                            let tab = rx_tab_state.borrow().clone();

                            if let TabState::Selected(id) = tab {
                                let chunk = InputChunk { data };
                                tx_request.send(Request::Input(id, chunk)).await?;
                            }
                        }
                        TerminalInput::Resize(size) => {
                            let tab = rx_tab_state.borrow().clone();

                            if let TabState::Selected(id) = tab {
                                debug!("setting size: {} {:?}", &id.0, &size);
                                tx_request.send(Request::ResizeTab(id, size)).await?;
                            }
                        }
                    }
                }

                Ok(())
            })
        };

        Ok(MainTerminalCarrier {
            _recv,
            _fuzzy_send,
            _forward_shutdown,
            _echo_output,
            _read_input,
        })
    }
}

impl TerminalBus {
    pub async fn await_selected(rx: &mut watch::Receiver<TabState>) {
        if let TabState::Selected(_) = *rx.borrow() {
            return;
        }

        while let Some(state) = rx.recv().await {
            if let TabState::Selected(_) = state {
                return;
            }
        }
    }
}
