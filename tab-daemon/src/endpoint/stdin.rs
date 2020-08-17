use super::Endpoint;
use crate::{pty_process::PtyRequest, session::DaemonSession};
use async_trait::async_trait;
use tab_api::{chunk::InputChunk, response::Response, tab::TabId};
use tokio::sync::mpsc::Sender;

pub struct StdinEndpoint;

#[async_trait]
impl Endpoint for StdinEndpoint {
    type Request = (TabId, InputChunk);

    async fn handle(
        session: &mut DaemonSession,
        (tab, data): Self::Request,
        _response_sink: Sender<Response>,
    ) -> anyhow::Result<()> {
        // check if the session is active
        // create an https://docs.rs/futures/0.3.5/futures/future/struct.Abortable.html
        // save in session, for termination
        if let Some(tab) = session.runtime().get_tab(tab.0 as usize).await {
            let pty_request = PtyRequest::Input(data);
            let mut sender = tab.pty_sender().clone();
            sender.send(pty_request).await?;
        } else {
            println!("No tab with id {:?}", tab);
        }

        Ok(())
    }
}
