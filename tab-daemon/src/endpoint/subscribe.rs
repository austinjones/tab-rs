use super::Endpoint;
use crate::session::DaemonSession;
use async_trait::async_trait;

use tab_api::{response::Response, tab::TabId};
use tokio::sync::mpsc::Sender;

pub struct SubscribeEndpoint;

#[async_trait]
impl Endpoint for SubscribeEndpoint {
    type Request = TabId;

    async fn handle(
        session: &mut DaemonSession,
        action: Self::Request,
        response_sink: Sender<Response>,
    ) -> anyhow::Result<()> {
        // check if the session is active
        // create an https://docs.rs/futures/0.3.5/futures/future/struct.Abortable.html
        // save in session, for termination
        session.subscribe(&action, response_sink).await?;
        Ok(())
    }
}
