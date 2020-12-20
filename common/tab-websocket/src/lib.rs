use std::path::Path;

use async_tungstenite::{
    tokio::{client_async, TokioAdapter},
    WebSocketStream,
};
use serde::{de::DeserializeOwned, Serialize};

use tokio::net::UnixStream;

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

pub type WebsocketConnection = WebSocketStream<TokioAdapter<UnixStream>>;

/// Connects to the provided URL, with no authentication token
pub async fn connect(
    socket: &Path,
    path: String,
) -> Result<WebsocketConnection, tungstenite::Error> {
    let conn = UnixStream::connect(socket).await?;
    let request = Request::builder()
        .uri(format!("ws://127.0.0.1{}", path))
        .body(())?;

    let tuple = client_async(request, conn).await?;
    Ok(tuple.0)
}

/// Connects to the provided URL, given an authentication token
pub async fn connect_authorized(
    socket: &Path,
    path: String,
    token: String,
) -> Result<WebsocketConnection, tungstenite::Error> {
    let conn = UnixStream::connect(socket).await?;

    let request = Request::builder()
        .uri(format!("ws://127.0.0.1{}", path))
        .header("Authorization", token.trim())
        .body(())?;

    let tuple = client_async(request, conn).await?;
    Ok(tuple.0)
}

/// Binds to the TCP stream as a server, requring the auth token, and capturing request metadata via a lifeline request.
pub async fn bind(
    unix: UnixStream,
    auth_token: WebsocketAuthToken,
    request_metadata: lifeline::request::Request<(), RequestMetadata>,
) -> Result<WebsocketConnection, tungstenite::Error> {
    let auth = AuthHandler::with_metadata(auth_token, Some(request_metadata));
    async_tungstenite::tokio::accept_hdr_async(unix, auth).await
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
