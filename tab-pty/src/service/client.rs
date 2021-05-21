use crate::message::pty::{MainShutdown, PtyOptions, PtyRequest, PtyResponse, PtyShutdown};
use crate::prelude::*;

use super::pty::PtyService;
use lifeline::dyn_bus::DynBus;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::PathBuf,
};
use tab_api::{
    config::history_path,
    env::is_raw_mode,
    pty::{PtyWebsocketRequest, PtyWebsocketResponse},
};
use time::Duration;
use tokio::time;

/// Drives messages between the pty, and the websocket connection to the daemon
/// Handles startup & shutdown events, including daemon termination commands.
/// Spawns the ClientSessionService, which handles the active tab session.
pub struct ClientService {
    _run: Lifeline,
    _carrier: MainPtyCarrier,
}

impl Service for ClientService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        // this needs to be above the carrier spawn
        // the pty_bus carrier forwards the websocket request messages,
        // and the init message can get lost in debug mode due to debug slowness
        let rx = bus.rx::<PtyWebsocketRequest>()?;

        let pty_bus = PtyBus::default();
        let _carrier = pty_bus.carry_from(bus)?;
        let tx_shutdown = bus.tx::<MainShutdown>()?;

        let _run = {
            let tx = bus.tx::<PtyWebsocketResponse>()?;
            Self::try_task("run", Self::run(rx, tx, tx_shutdown, pty_bus))
        };

        Ok(Self { _run, _carrier })
    }
}

impl ClientService {
    async fn run(
        mut rx: impl Stream<Item = PtyWebsocketRequest> + Unpin,
        mut tx: impl Sink<Item = PtyWebsocketResponse> + Clone + Unpin + Send + 'static,
        mut tx_shutdown: impl Sink<Item = MainShutdown> + Unpin,
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

                    let mut env = create.env.clone();
                    env.insert("SHELL".to_string(), create.shell.clone());
                    env.insert("TAB".to_string(), create.name.clone());
                    env.insert("TAB_ID".to_string(), create.id.0.to_string());

                    let shell = resolve_shell(create.shell.as_str());
                    debug!("shell detection: {:?}", shell);

                    // configure history files
                    match shell {
                        Shell::Sh => {
                            let home = history_path("sh", create.name.as_str())?;
                            std::fs::create_dir_all(home.parent().unwrap())?;

                            env.insert("HISTFILE".to_string(), home.to_string_lossy().to_string());
                        }
                        Shell::Zsh => {
                            // this doesn't work on OSX.  /etc/zshrc overwrites it
                            let home = history_path("zsh", create.name.as_str())?;
                            std::fs::create_dir_all(home.parent().unwrap())?;

                            env.insert("HISTFILE".to_string(), home.to_string_lossy().to_string());
                        }
                        Shell::Bash => {
                            let home = history_path("bash", create.name.as_str())?;
                            std::fs::create_dir_all(home.parent().unwrap())?;

                            env.insert("HISTFILE".to_string(), home.to_string_lossy().to_string());
                        }
                        Shell::Fish => {
                            let mut hasher = DefaultHasher::new();
                            name.hash(&mut hasher);
                            let id = hasher.finish();

                            let history = format!("tab_{}", id);

                            env.insert("fish_history".to_string(), history);
                        }
                        Shell::Unknown => {}
                    }

                    let mut args = vec![];

                    // configure shell args
                    match shell {
                        Shell::Sh => {
                            args.push("-l".to_string());
                        }
                        Shell::Zsh => {
                            args.push("--login".to_string());
                        }
                        Shell::Bash => {
                            args.push("--login".to_string());
                        }
                        Shell::Fish => {
                            args.push("--interactive".to_string());
                            args.push("--login".to_string());
                        }
                        Shell::Unknown => {}
                    }

                    if !is_raw_mode() {
                        // if we are in test mode, try to make the terminal as predictable as possible
                        info!("Raw mode is disabled.  Launching in non-interactive debug mode.");
                        env.insert("PS1".into(), "$ ".into());
                        if let Shell::Bash = shell {
                            args.push("--noprofile".into());
                            args.push("--norc".into());
                            args.push("--noediting".into());
                            env.insert("BASH_SILENCE_DEPRECATION_WARNING".into(), "1".into());
                        }
                    }

                    let working_directory = PathBuf::from(create.dir.clone());
                    let options = PtyOptions {
                        dimensions: create.dimensions,
                        command: create.shell.clone(),
                        args,
                        working_directory: working_directory.clone(),
                        env,
                    };

                    pty_bus.store_resource::<PtyOptions>(options);
                    let session = ClientSessionService::spawn(&pty_bus)?;
                    _session = Some(session);

                    debug!("tab initialized, name {}", name);
                    tx.send(PtyWebsocketResponse::Started(create)).await?;
                }
                PtyWebsocketRequest::Input(_) => {}
                PtyWebsocketRequest::Resize(_) => {}
                PtyWebsocketRequest::Terminate => {
                    // in case we somehow get a pty termination request, but don't have a session running,
                    // send a main shutdown message
                    time::sleep(Duration::from_millis(2000)).await;
                    tx.send(PtyWebsocketResponse::Stopped).await.ok();
                    tx_shutdown.send(MainShutdown {}).await?;
                }
            }
        }

        Ok(())
    }
}

/// Drives an active tab session, forwarding input/output events betweeen the pty & daemon.
/// Handles termination requests (from the daemon), and termination events (from the pty).
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
            let tx_websocket = bus.tx::<PtyWebsocketResponse>()?;
            let tx_shutdown = bus.tx::<PtyShutdown>()?;
            Self::try_task(
                "input",
                Self::input(rx_request, tx_pty, tx_websocket, tx_shutdown),
            )
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
        mut rx: impl Stream<Item = PtyWebsocketRequest> + Unpin,
        mut tx_pty: impl Sink<Item = PtyRequest> + Unpin,
        mut tx_websocket: impl Sink<Item = PtyWebsocketResponse> + Unpin,
        mut tx_shutdown: impl Sink<Item = PtyShutdown> + Unpin,
    ) -> anyhow::Result<()> {
        while let Some(request) = rx.recv().await {
            match request {
                PtyWebsocketRequest::Input(input) => {
                    let message = PtyRequest::Input(input);

                    tx_pty.send(message).await.ok();
                }
                PtyWebsocketRequest::Resize(dimensions) => {
                    debug!("received resize request: {:?}", dimensions);

                    tx_pty.send(PtyRequest::Resize(dimensions)).await.ok();
                }
                PtyWebsocketRequest::Terminate => {
                    info!("Terminating due to command request.");

                    tx_pty.send(PtyRequest::Shutdown).await.ok();

                    // The shell should shut down, and emit a shutdown message.
                    // If it doesn't within a reasonable time,
                    //   we'll forcefully kill it.
                    time::sleep(Duration::from_millis(1000)).await;
                    warn!("Shell process did not shut down within the 1 second timeout.");
                    tx_websocket.send(PtyWebsocketResponse::Stopped).await?;
                    tx_shutdown.send(PtyShutdown {}).await?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn output(
        mut rx: impl Stream<Item = PtyResponse> + Unpin,
        mut tx: impl Sink<Item = PtyWebsocketResponse> + Unpin,
        mut tx_shutdown: impl Sink<Item = PtyShutdown> + Unpin,
    ) -> anyhow::Result<()> {
        while let Some(msg) = rx.recv().await {
            match msg {
                PtyResponse::Output(out) => {
                    tx.send(PtyWebsocketResponse::Output(out)).await?;
                }
                PtyResponse::Terminated => {
                    debug!("pty child process terminated");

                    tx.send(PtyWebsocketResponse::Stopped).await?;

                    // this sleep is not visible to the user
                    time::sleep(Duration::from_millis(100)).await;
                    tx_shutdown.send(PtyShutdown {}).await?;
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shell {
    Sh,
    Zsh,
    Bash,
    Fish,
    Unknown,
}

pub fn resolve_shell(command: &str) -> Shell {
    for fragment in command.split(|c| c == '/' || c == ' ' || c == '\\') {
        let fragment = fragment.trim();
        if fragment.eq_ignore_ascii_case("sh") {
            return Shell::Sh;
        } else if fragment.eq_ignore_ascii_case("zsh") {
            return Shell::Zsh;
        } else if fragment.eq_ignore_ascii_case("bash") {
            return Shell::Bash;
        } else if fragment.eq_ignore_ascii_case("fish") {
            return Shell::Fish;
        }
    }

    Shell::Unknown
}

#[cfg(test)]
mod shell_test {
    use super::{resolve_shell, Shell};

    #[test]
    fn test_shell() {
        assert_eq!(Shell::Sh, resolve_shell("sh"));
        assert_eq!(Shell::Zsh, resolve_shell("zsh"));
        assert_eq!(Shell::Bash, resolve_shell("bash"));
        assert_eq!(Shell::Fish, resolve_shell("fish"));

        assert_eq!(Shell::Unknown, resolve_shell("batty"));
    }

    #[test]
    fn test_absolute_shell() {
        assert_eq!(Shell::Sh, resolve_shell("/bin/sh"));
    }

    #[test]
    fn test_relative_shell() {
        assert_eq!(Shell::Sh, resolve_shell("./sh"));
    }

    #[test]
    fn test_env_shell() {
        assert_eq!(Shell::Sh, resolve_shell("/usr/bin/env sh"));
    }

    #[test]
    fn test_shell_args() {
        assert_eq!(Shell::Sh, resolve_shell("/bin/sh --flag -f"));
        assert_eq!(Shell::Sh, resolve_shell("/usr/bin/env sh --flag -f"));
        assert_eq!(Shell::Sh, resolve_shell("sh --flag -f"));
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, time::Duration};

    use lifeline::{assert_completes, assert_times_out};
    use postage::{sink::Sink, stream::Stream};
    use tab_api::{
        pty::{PtyWebsocketRequest, PtyWebsocketResponse},
        tab::TabId,
        tab::TabMetadata,
    };
    use tokio::time;

    use super::ClientService;
    use crate::{message::pty::MainShutdown, prelude::*};

    #[tokio::test]
    async fn launch_sh() -> anyhow::Result<()> {
        let bus = MainBus::default();
        let _service = ClientService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyWebsocketRequest>()?;
        let mut rx = bus.rx::<PtyWebsocketResponse>()?;

        let current_dir = std::env::current_dir().unwrap();
        tx.send(PtyWebsocketRequest::Init(TabMetadata {
            id: TabId(0),
            name: "name".into(),
            doc: Some("doc".into()),
            dimensions: (80, 24),
            env: HashMap::new(),
            shell: "/usr/bin/env sh".into(),
            dir: current_dir.to_string_lossy().into(),
            selected: 0,
        }))
        .await?;

        assert_completes!(async move {
            let created = rx.recv().await;
            assert_eq!(
                Some(PtyWebsocketResponse::Started(TabMetadata {
                    id: TabId(0),
                    name: "name".into(),
                    doc: Some("doc".into()),
                    dimensions: (80, 24),
                    env: HashMap::new(),
                    shell: "/usr/bin/env sh".into(),
                    dir: current_dir.to_string_lossy().into(),
                    selected: 0,
                })),
                created
            );
        });

        Ok(())
    }

    #[tokio::test]
    async fn terminate_escape() -> anyhow::Result<()> {
        let bus = MainBus::default();
        let _service = ClientService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyWebsocketRequest>()?;
        let mut rx = bus.rx::<PtyWebsocketResponse>()?;
        let mut rx_shutdown = bus.rx::<MainShutdown>()?;

        tx.send(PtyWebsocketRequest::Terminate).await?;

        assert_times_out!(
            async {
                rx.recv().await;
            },
            1900
        );

        // wait for a total of 2000ms + 10ms.
        time::sleep(Duration::from_millis(110)).await;

        assert_completes!(async {
            let msg = rx.recv().await;
            assert_eq!(Some(PtyWebsocketResponse::Stopped), msg)
        });

        assert_completes!(async {
            let msg = rx_shutdown.recv().await;
            assert_eq!(Some(MainShutdown {}), msg);
        });

        Ok(())
    }
}

#[cfg(test)]
mod client_session_tests {
    use lifeline::{assert_completes, assert_times_out, dyn_bus::DynBus};
    use postage::{sink::Sink, stream::Stream};
    use tab_api::{
        chunk::InputChunk, chunk::OutputChunk, pty::PtyWebsocketRequest, pty::PtyWebsocketResponse,
    };
    use tokio::time;

    use crate::{
        message::pty::PtyOptions, message::pty::PtyRequest, message::pty::PtyResponse,
        message::pty::PtyShutdown, prelude::*,
    };
    use std::{collections::HashMap, time::Duration};

    use super::ClientSessionService;

    fn options() -> PtyOptions {
        let current_dir = std::env::current_dir().unwrap();

        PtyOptions {
            working_directory: current_dir,
            dimensions: (80, 24),
            command: "/usr/bin/env sh".to_string(),
            args: vec![],
            env: HashMap::new(),
        }
    }
    #[tokio::test]
    async fn rx_input() -> anyhow::Result<()> {
        let bus = PtyBus::default();
        bus.store_resource::<PtyOptions>(options());

        let _service = ClientSessionService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyWebsocketRequest>()?;
        let mut rx = bus.rx::<PtyRequest>()?;

        let input = InputChunk { data: vec![0, 1] };

        tx.send(PtyWebsocketRequest::Input(input.clone())).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(PtyRequest::Input(input)), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn rx_resize() -> anyhow::Result<()> {
        let bus = PtyBus::default();
        bus.store_resource::<PtyOptions>(options());

        let _service = ClientSessionService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyWebsocketRequest>()?;
        let mut rx = bus.rx::<PtyRequest>()?;

        tx.send(PtyWebsocketRequest::Resize((1, 2))).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(PtyRequest::Resize((1, 2))), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn rx_terminate() -> anyhow::Result<()> {
        let bus = PtyBus::default();
        bus.store_resource::<PtyOptions>(options());

        let _service = ClientSessionService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyWebsocketRequest>()?;
        let mut rx = bus.rx::<PtyRequest>()?;
        let mut rx_websocket = bus.rx::<PtyWebsocketResponse>()?;
        let mut rx_shutdown = bus.rx::<PtyShutdown>()?;

        tx.send(PtyWebsocketRequest::Terminate).await?;

        assert_completes!(
            async move {
                let msg = rx.recv().await;
                assert_eq!(Some(PtyRequest::Shutdown), msg);
            },
            15
        );

        assert_times_out!(async {
            rx_websocket.recv().await;
        });

        assert_times_out!(async {
            rx_shutdown.recv().await;
        });

        time::sleep(Duration::from_millis(1000)).await;

        assert_completes!(async {
            let msg = rx_websocket.recv().await;
            assert_eq!(Some(PtyWebsocketResponse::Stopped), msg)
        });

        assert_completes!(async {
            let msg = rx_shutdown.recv().await;
            assert_eq!(Some(PtyShutdown {}), msg)
        });

        Ok(())
    }

    #[tokio::test]
    async fn tx_output() -> anyhow::Result<()> {
        let bus = PtyBus::default();
        bus.store_resource::<PtyOptions>(options());

        let _service = ClientSessionService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyResponse>()?;
        let mut rx = bus.rx::<PtyWebsocketResponse>()?;

        let output = OutputChunk {
            index: 0,
            data: vec![0, 1],
        };

        tx.send(PtyResponse::Output(output.clone())).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(PtyWebsocketResponse::Output(output)), msg);
        });

        Ok(())
    }

    #[tokio::test]
    async fn tx_terminated() -> anyhow::Result<()> {
        let bus = PtyBus::default();
        bus.store_resource::<PtyOptions>(options());

        let _service = ClientSessionService::spawn(&bus)?;

        let mut tx = bus.tx::<PtyResponse>()?;
        let mut rx = bus.rx::<PtyWebsocketResponse>()?;
        let mut rx_shutdown = bus.rx::<PtyShutdown>()?;

        tx.send(PtyResponse::Terminated).await?;

        assert_completes!(async move {
            let msg = rx.recv().await;
            assert_eq!(Some(PtyWebsocketResponse::Stopped), msg);
        });

        assert_times_out!(
            async {
                rx_shutdown.recv().await;
            },
            90
        );

        time::sleep(Duration::from_millis(20)).await;

        assert_completes!(
            async {
                rx_shutdown.recv().await;
            },
            90
        );

        Ok(())
    }
}
