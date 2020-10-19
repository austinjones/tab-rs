use crate::{
    message::main::MainRecv, message::main::MainShutdown, prelude::*,
    state::workspace::WorkspaceState, utils::await_state,
};

pub struct MainCheckWorkspaceService {
    _run: Lifeline,
}

impl Service for MainCheckWorkspaceService {
    type Bus = MainBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let mut rx = bus.rx::<MainRecv>()?;
        let mut rx_workspace = bus.rx::<Option<WorkspaceState>>()?.into_inner();

        let mut tx_shutdown = bus.tx::<MainShutdown>()?;

        let _run = Self::try_task("run", async move {
            while let Some(msg) = rx.recv().await {
                if let MainRecv::CheckWorkspace = msg {
                    let workspace = await_state(&mut rx_workspace).await?;

                    Self::echo_errors(&workspace.errors);
                    tx_shutdown.send(MainShutdown {}).await.ok();
                    break;
                }
            }

            Ok(())
        });

        Ok(Self { _run })
    }
}

impl MainCheckWorkspaceService {
    fn echo_errors(errors: &Vec<String>) {
        if errors.len() == 0 {
            eprintln!("No errors detected.");
            return;
        } else if errors.len() == 1 {
            eprintln!("{} error was detected:", errors.len());
        } else {
            eprintln!("{} errors were detected:", errors.len());
        }

        for error in errors {
            eprintln!("    - {}", error);
        }
    }
}
