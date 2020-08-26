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

            Self::try_task("main_recv", async move {
                while let Some(msg) = rx_main.recv().await {
                    match msg {
                        MainRecv::SelectInteractive => {
                            tx_terminal_mode.send(TerminalMode::Crossterm).await?;
                        }
                        MainRecv::SelectTab(_) => {
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
                while let None = rx_shutdown.recv().await {}
                tx_shutdown.send(MainShutdown {}).await?;
                Ok(())
            })
        };

        let _echo_output = {
            let rx_tab_state = from.rx::<TabState>()?.into_inner();
            let mut rx_response = from.rx::<Response>()?;
            let mut tx_output = self.tx::<TerminalRecv>()?;

            Self::try_task("main_recv", async move {
                while let Some(response) = rx_response.recv().await {
                    match response {
                        Response::Output(tab_id, stdout) => {
                            if rx_tab_state.borrow().is_selected(&tab_id) {
                                tx_output
                                    .send(TerminalRecv::Stdout(stdout.data))
                                    .await
                                    .context("tx TerminalRecv::Stdout")?;
                            }
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

                            if let TabState::Selected(id, _name) = tab {
                                let chunk = InputChunk { data };
                                tx_request.send(Request::Input(id, chunk)).await?;
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
