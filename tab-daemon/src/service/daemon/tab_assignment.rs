use std::time::{Duration, Instant};

use anyhow::Context;
use tab_api::launch::launch_pty;
use tokio::time;

use crate::{
    message::tab::TabRecv, message::tab_assignment::AssignTab,
    message::tab_assignment::TabAssignmentRetraction, prelude::*, state::assignment::assignment,
};

const SPAWN_DELAY: Duration = Duration::from_millis(500);

pub struct TabAssignmentService {
    _recv_assign: Lifeline,
    _reassign: Lifeline,
    _spawn_pty: Lifeline,
}

impl Service for TabAssignmentService {
    type Bus = ListenerBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let _recv_assign = {
            let mut rx = bus.rx::<AssignTab>()?;
            let mut tx_retraction = bus.tx::<TabAssignmentRetraction>()?;
            let mut tx_tabs = bus.tx::<TabRecv>()?;

            Self::try_task("recv_assign", async move {
                while let Some(assign) = rx.recv().await {
                    debug!("generating assignment for tab {:?}", assign.0.id);
                    let (ret, assign) = assignment(assign.0);
                    let message = TabRecv::Assign(assign);
                    tx_tabs.send(message).await.ok();

                    let retraction = TabAssignmentRetraction(ret);
                    tx_retraction
                        .send(retraction)
                        .await
                        .context("tx_tab_retraction send message")?;
                }
                Ok(())
            })
        };

        let _reassign = {
            let mut rx = bus.rx::<TabAssignmentRetraction>()?;
            let mut tx_assign = bus.tx::<AssignTab>()?;
            Self::try_task("reassign", async move {
                'retractions: while let Some(retraction) = rx.recv().await {
                    let retraction = retraction.0;
                    let mut retracted = retraction.retract_if_expired(Duration::from_millis(25));

                    while let None = retracted {
                        if retraction.is_taken() {
                            continue 'retractions;
                        }

                        retracted = retraction.retract_if_expired(Duration::from_millis(25));
                        time::sleep(Duration::from_millis(5)).await;
                    }

                    let metadata = retracted.unwrap();
                    tx_assign.send(AssignTab(metadata)).await?;
                }

                Ok(())
            })
        };

        let _spawn_pty = {
            let mut rx = bus.rx::<TabAssignmentRetraction>()?;
            Self::try_task("spawn_pty", async move {
                let mut last_spawn: Option<Instant> = None;
                while let Some(_) = rx.recv().await {
                    if last_spawn
                        .map(|inst| Instant::now().duration_since(inst) > SPAWN_DELAY)
                        .unwrap_or(true)
                    {
                        debug!("launching pty process");
                        if let Err(e) = launch_pty() {
                            error!("failed to launch initial pty process: {}", e);
                        }

                        while let Ok(_) = rx.try_recv() {
                            debug!("launching pty process");
                            if let Err(e) = launch_pty() {
                                error!("failed to launch pty process: {}", e);
                            }
                        }

                        last_spawn = Some(Instant::now());
                    }
                }

                Ok(())
            })
        };

        Ok(Self {
            _recv_assign,
            _reassign,
            _spawn_pty,
        })
    }
}
