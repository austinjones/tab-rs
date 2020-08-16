use super::Endpoint;
use crate::session::DaemonSession;
use async_trait::async_trait;
use tab_api::{
    chunk::StdinChunk,
    response::Response,
    tab::{CreateTabMetadata, TabId, TabMetadata},
};
use tokio::{runtime::Runtime, sync::mpsc::Sender};

pub struct StdinEndpoint;

#[async_trait]
impl Endpoint for StdinEndpoint {
    type Request = (TabId, StdinChunk);

    async fn handle(
        session: &mut DaemonSession,
        (tab, data): Self::Request,
        mut response_sink: Sender<Response>,
    ) -> anyhow::Result<()> {
        // check if the session is active
        // create an https://docs.rs/futures/0.3.5/futures/future/struct.Abortable.html
        // save in session, for termination
        if let Some(tab) = session.runtime().get_tab(tab.0 as usize).await {
            tab.process().write(data).await?;
        } else {
            println!("No tab with id {:?}", tab);
        }

        Ok(())
    }
}
