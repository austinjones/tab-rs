use crate::{prelude::*, state::tab::TabState};

use super::terminal::set_title;

pub struct TabTitleService {
    _title: Lifeline,
}

impl Service for TabTitleService {
    type Bus = TabBus;
    type Lifeline = anyhow::Result<Self>;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline {
        let rx = bus.rx::<TabState>()?;
        let _title = Self::try_task("title", Self::forward_title(rx));
        Ok(Self { _title })
    }
}

impl TabTitleService {
    async fn forward_title(mut rx: impl Receiver<TabState>) -> anyhow::Result<()> {
        info!("debug title service starting...");
        while let Some(msg) = rx.recv().await {
            if let TabState::Selected(state) = msg {
                let title = format!("tab {}", state.name);

                if let Err(e) = set_title(title.as_str()) {
                    warn!("failed to set terminal title: {}", e);
                }
            }
        }

        Ok(())
    }
}
