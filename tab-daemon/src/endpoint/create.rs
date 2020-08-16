use super::Endpoint;
use crate::session::DaemonSession;
use async_trait::async_trait;
use tab_api::{
    response::Response,
    tab::{CreateTabMetadata, TabMetadata},
};
use tokio::{runtime::Runtime, sync::mpsc::Sender};

pub struct CreateTabEndpoint;

#[async_trait]
impl Endpoint for CreateTabEndpoint {
    type Request = CreateTabMetadata;

    async fn handle(
        session: &mut DaemonSession,
        action: &Self::Request,
        mut response_sink: Sender<Response>,
    ) -> anyhow::Result<()> {
        // check if the session is active
        // create an https://docs.rs/futures/0.3.5/futures/future/struct.Abortable.html
        // save in session, for termination
        if let Some(tab) = session.runtime().find_tab(action.name.as_str()).await {
            let metadata: TabMetadata = tab.metadata().clone();
            response_sink.send(Response::TabUpdate(metadata)).await?;
            session.subscribe(&tab.id(), response_sink).await?;
            return Ok(());
        }

        let tab = session.runtime().create_tab(action).await?;
        response_sink
            .send(Response::TabUpdate(tab.metadata().clone()))
            .await?;

        session.subscribe(&tab.id(), response_sink).await?;

        Ok(())
    }
}
