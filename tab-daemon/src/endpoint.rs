use crate::session::DaemonSession;
use async_trait::async_trait;
use create::CreateTabEndpoint;
use futures::Sink;
use log::info;
use std::fmt::Debug;
use subscribe::SubscribeEndpoint;
use tab_api::{request::Request, response::Response};
use tokio::sync::mpsc::Sender;

mod create;
mod subscribe;

#[async_trait]
trait Endpoint {
    type Request;
    async fn handle(
        session: &mut DaemonSession,
        action: &Self::Request,
        response_sink: Sender<Response>,
    ) -> anyhow::Result<()>;
}

pub async fn handle_request(
    request: Request,
    session: &mut DaemonSession,
    mut response_sink: Sender<Response>,
) -> anyhow::Result<()> {
    let description = format!("{:?}", request);
    info!("start request: {:?}", description);

    match request {
        Request::Auth(_) => {
            // TODO: implement authentication.  it should take more than a socket connection to execute.
            // maybe a random key saved in the daemonfile?
        }
        Request::Subscribe(tab) => SubscribeEndpoint::handle(session, &tab, response_sink).await?,
        Request::Unsubscribe(tab) => {}
        Request::Stdin(tab, data) => unimplemented!(),
        Request::CreateTab(metadata) => {
            CreateTabEndpoint::handle(session, &metadata, response_sink).await?
        }
        Request::CloseTab(tab) => unimplemented!(),
        Request::ListTabs => {
            response_sink.send(Response::TabList(vec![])).await?;
        }
    }

    info!("end request: {:?}", description);

    Ok(())
}
