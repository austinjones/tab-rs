use crate::{
    message::main::MainRecv, message::terminal::TerminalRecv, message::terminal::TerminalSend,
    prelude::*, state::workspace::WorkspaceState,
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
use postage::{broadcast, mpsc, watch};
use tab_api::chunk::InputChunk;

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

impl Message<TerminalBus> for TerminalShutdown {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TerminalBus> for TerminalSend {
    type Channel = mpsc::Sender<Self>;
}

impl Message<TerminalBus> for TerminalMode {
    type Channel = watch::Sender<Self>;
}

impl Message<TerminalBus> for Option<WorkspaceState> {
    type Channel = watch::Sender<Self>;
}

/// Carries messages between the MainBus, and the TerminalBus.
///
/// Listens to MainRecv and sends TerminalMode,
/// forwards TerminalShutdown, and carries Input, Output, and Resize events.
pub struct MainTerminalCarrier {
    _recv: Lifeline,
    _send: Lifeline,
    _forward_shutdown: Lifeline,
    _forward_workspace: Lifeline,
    _echo_output: Lifeline,
    _read_input: Lifeline,
}

impl CarryFrom<MainBus> for TerminalBus {
    type Lifeline = anyhow::Result<MainTerminalCarrier>;

    fn carry_from(&self, from: &MainBus) -> Self::Lifeline {
        let _recv = {
            let mut rx = from.rx::<TerminalRecv>()?;
            let mut tx_terminal_mode = self.tx::<TerminalMode>()?;

            Self::try_task("recv", async move {
                while let Some(msg) = rx.recv().await {
                    match msg {
                        TerminalRecv::Mode(mode) => tx_terminal_mode.send(mode).await?,
                    }
                }

                Ok(())
            })
        };

        let _send = {
            let mut rx = self.rx::<TerminalSend>()?;
            let mut tx = from.tx::<MainRecv>()?;

            Self::try_task("send", async move {
                while let Some(msg) = rx.recv().await {
                    match msg {
                        TerminalSend::FuzzyRequest => tx.send(MainRecv::SelectInteractive).await?,
                        TerminalSend::FuzzySelection(selection) => {
                            tx.send(MainRecv::SelectTab(selection)).await?
                        }
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
                    tx_shutdown.send(MainShutdown(0)).await?;
                }

                Ok(())
            })
        };

        let _forward_workspace = {
            let mut rx = from.rx::<Option<WorkspaceState>>()?;
            let mut tx = self.tx::<Option<WorkspaceState>>()?;

            Self::try_task("recv", async move {
                while let Some(msg) = rx.recv().await {
                    tx.send(msg).await?;
                }

                Ok(())
            })
        };

        let _echo_output = {
            let mut rx_response = from.rx::<Response>()?;
            let mut tx_output = self.tx::<TerminalOutput>()?;

            Self::try_task("main_recv", async move {
                while let Some(response) = rx_response.recv().await {
                    if let Response::Output(_id, stdout) = response {
                        tx_output
                            .send(TerminalOutput::Stdout(stdout.data))
                            .await
                            .context("tx TerminalRecv::Stdout")?;
                    }
                }

                Ok(())
            })
        };

        let _read_input = {
            let rx_tab_state = from.rx::<TabState>()?;
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
            _send,
            _forward_shutdown,
            _forward_workspace,
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
