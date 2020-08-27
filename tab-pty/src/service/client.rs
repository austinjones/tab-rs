use crate::message::pty::{MainShutdown, PtyOptions, PtyRequest, PtyResponse, PtyShutdown};
use crate::prelude::*;

use super::pty::PtyService;
use lifeline::dyn_bus::DynBus;
use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
};
use tab_api::{
    config::history_path,
    pty::{PtyWebsocketRequest, PtyWebsocketResponse},
};
use time::Duration;
use tokio::time;

pub struct ClientService {
    _run: Lifeline,
    _carrier: MainPtyCarrier,
}

impl Service for ClientService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let pty_bus = PtyBus::default();
        let _carrier = pty_bus.carry_from(bus)?;
        let tx_shutdown = bus.tx::<MainShutdown>()?;

        let _run = {
            let rx = bus.rx::<PtyWebsocketRequest>()?;
            let tx = bus.tx::<PtyWebsocketResponse>()?;
            Self::try_task("run", Self::run(rx, tx, tx_shutdown, pty_bus))
        };

        Ok(Self { _run, _carrier })
    }
}

impl ClientService {
    async fn run(
        mut rx: impl Receiver<PtyWebsocketRequest>,
        mut tx: impl Sender<PtyWebsocketResponse> + Clone + Send + 'static,
        mut tx_shutdown: impl Sender<MainShutdown>,
        pty_bus: PtyBus,
    ) -> anyhow::Result<()> {
        // TODO: handle ptyshutdown here.
        // it should cancel the session lifeline
        let mut _session = None;
        while let Some(msg) = rx.recv().await {
            match msg {
                PtyWebsocketRequest::Init(create) => {
                    debug!("initializing on tab {}", create.id);
                    let name = create.name.clone();

                    let mut env = HashMap::new();
                    env.insert("SHELL".to_string(), create.shell.clone());
                    env.insert("TAB".to_string(), create.name.clone());
                    env.insert("TAB_ID".to_string(), create.id.0.to_string());

                    // todo: better resolution of shells
                    if create.shell.ends_with("fish") {
                        let mut hasher = DefaultHasher::new();
                        name.hash(&mut hasher);
                        let id = hasher.finish();

                        let history = format!("tab_{}", id);

                        env.insert("fish_history".to_string(), history);
                    } else if create.shell.ends_with("bash") {
                        let home = history_path("fish", create.name.as_str())?;
                        std::fs::create_dir_all(home.parent().unwrap())?;

                        env.insert("HISTFILE".to_string(), home.to_string_lossy().to_string());
                    } else if create.shell.ends_with("zsh") {
                        // this doesn't work on OSX.  /etc/zshrc overwrites it
                        let home = history_path("zsh", create.name.as_str())?;
                        std::fs::create_dir_all(home.parent().unwrap())?;

                        env.insert("HISTFILE".to_string(), home.to_string_lossy().to_string());
                    }

                    let options = PtyOptions {
                        dimensions: create.dimensions,
                        command: create.shell.clone(),
                        env,
                    };

                    pty_bus.store_resource::<PtyOptions>(options);
                    let session = ClientSessionService::spawn(&pty_bus)?;
                    _session = Some(session);

                    info!("tab initialized, name {}", name);
                    tx.send(PtyWebsocketResponse::Started(create)).await?;
                }
                PtyWebsocketRequest::Input(_) => {}
                PtyWebsocketRequest::Resize(_) => {}
                PtyWebsocketRequest::Terminate => {
                    // in case we somehow get a pty termination request, but don't have a session running,
                    // send a main shutdown message
                    time::delay_for(Duration::from_millis(100)).await;
                    tx_shutdown.send(MainShutdown {}).await?;
                }
            }
        }

        Ok(())
    }
}

pub struct ClientSessionService {
    _pty: PtyService,
    _output: Lifeline,
    _input: Lifeline,
}

impl Service for ClientSessionService {
    type Bus = PtyBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let _pty = PtyService::spawn(&bus)?;

        let _output = {
            let rx_response = bus.rx::<PtyResponse>()?;
            let tx_websocket = bus.tx::<PtyWebsocketResponse>()?;
            let tx_shutdown = bus.tx::<PtyShutdown>()?;
            Self::try_task(
                "output",
                Self::output(rx_response, tx_websocket, tx_shutdown),
            )
        };

        let _input = {
            let rx_request = bus.rx::<PtyWebsocketRequest>()?;
            let tx_pty = bus.tx::<PtyRequest>()?;
            let tx_shutdown = bus.tx::<PtyShutdown>()?;
            Self::try_task("input", Self::input(rx_request, tx_pty, tx_shutdown))
        };

        Ok(Self {
            _pty,
            _output,
            _input,
        })
    }
}

impl ClientSessionService {
    async fn input(
        mut rx: impl Receiver<PtyWebsocketRequest>,
        mut tx_pty: impl Sender<PtyRequest>,
        mut tx_shutdown: impl Sender<PtyShutdown>,
    ) -> anyhow::Result<()> {
        while let Some(request) = rx.recv().await {
            match request {
                PtyWebsocketRequest::Input(input) => {
                    let message = PtyRequest::Input(input);

                    tx_pty.send(message).await.ok();
                }
                PtyWebsocketRequest::Terminate => {
                    info!("received termination request over websocket. terminating.");

                    tx_pty.send(PtyRequest::Shutdown).await.ok();

                    time::delay_for(Duration::from_millis(20)).await;
                    tx_shutdown.send(PtyShutdown {}).await?;
                }
                PtyWebsocketRequest::Resize(dimensions) => {
                    debug!("received resize request: {:?}", dimensions);

                    tx_pty.send(PtyRequest::Resize(dimensions)).await.ok();
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn output(
        mut rx: impl Receiver<PtyResponse>,
        mut tx: impl Sender<PtyWebsocketResponse>,
        mut tx_shutdown: impl Sender<PtyShutdown>,
    ) -> anyhow::Result<()> {
        while let Some(msg) = rx.recv().await {
            match msg {
                PtyResponse::Output(out) => {
                    tx.send(PtyWebsocketResponse::Output(out)).await?;
                }
                PtyResponse::Terminated(_term) => {
                    tx.send(PtyWebsocketResponse::Stopped).await?;
                    tx_shutdown.send(PtyShutdown {}).await?;
                }
            }
        }

        Ok(())
    }
}
