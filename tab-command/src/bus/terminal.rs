use crate::prelude::*;
use crate::{
    message::{
        main::{MainRecv, MainShutdown},
        terminal::{TerminalRecv, TerminalSend, TerminalShutdown},
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

impl Message<TerminalBus> for TerminalSend {
    type Channel = broadcast::Sender<Self>;
}

impl Message<TerminalBus> for TerminalRecv {
    type Channel = broadcast::Sender<Self>;
}

impl Message<TerminalBus> for TerminalSizeState {
    type Channel = watch::Sender<Self>;
}

impl Message<TerminalBus> for TerminalMode {
    type Channel = watch::Sender<Self>;
}

impl Message<TerminalBus> for TerminalShutdown {
    type Channel = mpsc::Sender<Self>;
}

/// Carries messages between the MainBus, and the TerminalBus.
///
/// Listens to MainRecv and sends TerminalMode,
/// forwards TerminalShutdown, and carries Input, Output, and Resize events.
pub struct MainTerminalCarrier {
    pub(super) _main: Lifeline,
    pub(super) _forward_shutdown: Lifeline,
    pub(super) _echo_output: Lifeline,
    pub(super) _read_input: Lifeline,
}

impl CarryFrom<MainBus> for TerminalBus {
    type Lifeline = anyhow::Result<MainTerminalCarrier>;

    fn carry_from(&self, from: &MainBus) -> Self::Lifeline {
        let _main = {
            let mut rx_main = from.rx::<MainRecv>()?;
            let mut tx_terminal_mode = self.tx::<TerminalMode>()?;
            let mut rx_tab_state = from.rx::<TabState>()?.into_inner();

            Self::try_task("main_recv", async move {
                while let Some(msg) = rx_main.recv().await {
                    match msg {
                        // MainRecv::SelectInteractive => {
                        //     tx_terminal_mode.send(TerminalMode::Crossterm).await?;
                        // }
                        MainRecv::SelectTab(_) => {
                            // we don't want to begin reading stdin until the tab has been selected
                            Self::await_selected(&mut rx_tab_state).await;
                            tx_terminal_mode.send(TerminalMode::Echo).await?;
                        }
                        _ => {}
                    }
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
            let mut tx_output = self.tx::<TerminalRecv>()?;

            Self::try_task("main_recv", async move {
                while let Some(response) = rx_response.recv().await {
                    match response {
                        Response::Output(_id, stdout) => {
                            tx_output
                                .send(TerminalRecv::Stdout(stdout.data))
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
            let mut rx_terminal_input = self.rx::<TerminalSend>()?;
            let mut tx_request = from.tx::<Request>()?;

            Self::try_task("main_recv", async move {
                while let Some(msg) = rx_terminal_input.recv().await {
                    match msg {
                        TerminalSend::Stdin(data) => {
                            let tab = rx_tab_state.borrow().clone();

                            if let TabState::Selected(tab) = tab {
                                let chunk = InputChunk { data };
                                tx_request.send(Request::Input(tab.id, chunk)).await?;
                            }
                        }
                        TerminalSend::Resize(size) => {
                            let tab = rx_tab_state.borrow().clone();

                            if let TabState::Selected(tab) = tab {
                                debug!("setting size: {} {:?}", tab.id.0, &size);
                                tx_request.send(Request::ResizeTab(tab.id, size)).await?;
                            }
                        }
                    }
                }

                Ok(())
            })
        };

        Ok(MainTerminalCarrier {
            _main,
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
