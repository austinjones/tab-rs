use async_tungstenite::{
    tokio::{connect_async, TokioAdapter},
    WebSocketStream,
};
use serde::{de::DeserializeOwned, Serialize};

use tokio::net::TcpStream;

use auth::AuthHandler;

use message::listener::RequestMetadata;
use resource::listener::WebsocketAuthToken;
use tungstenite::{handshake::client::Request, Message};
mod auth;
pub mod bus;
mod common;
pub mod message;
pub mod resource;
pub mod service;

pub type WebsocketConnection = WebSocketStream<TokioAdapter<TcpStream>>;

/// Connects to the provided URL, with no authentication token
pub async fn connect(url: String) -> Result<WebsocketConnection, tungstenite::Error> {
    let tuple = connect_async(url).await?;
    Ok(tuple.0)
}

/// Connects to the provided URL, given an authentication token
pub async fn connect_authorized(
    url: String,
    token: String,
) -> Result<WebsocketConnection, tungstenite::Error> {
    let request = Request::builder()
        .uri(url)
        .header("Authorization", token.trim())
        .body(())?;

    let (stream, _resp) = connect_async(request).await?;
    Ok(stream)
}

/// Binds to the TCP stream as a server, requring the auth token, and capturing request metadata via a lifeline request.
pub async fn bind(
    tcp: TcpStream,
    auth_token: WebsocketAuthToken,
    request_metadata: lifeline::request::Request<(), RequestMetadata>,
) -> Result<WebsocketConnection, tungstenite::Error> {
    let auth = AuthHandler::with_metadata(auth_token, Some(request_metadata));
    async_tungstenite::tokio::accept_hdr_async(tcp, auth).await
}

/// Decodes the bincode-serialized message
pub fn decode<T: DeserializeOwned>(
    message: Result<tungstenite::Message, tungstenite::Error>,
) -> anyhow::Result<T> {
    let message = message?;
    let data = bincode::deserialize::<T>(message.into_data().as_slice())?;
    Ok(data)
}

/// Encodes the message into a bincode-serialized tungstenite message
pub fn encode<T: Serialize>(message: T) -> anyhow::Result<tungstenite::Message> {
    let message = bincode::serialize(&message)?;
    Ok(Message::Binary(message))
}
