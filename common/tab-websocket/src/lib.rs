use async_tungstenite::{
    tokio::{connect_async, TokioAdapter},
    WebSocketStream,
};
use futures::Future;
use serde::{de::DeserializeOwned, Serialize};

use tokio::net::TcpStream;

use auth::AuthHandler;
use resource::listener::WebsocketAuthToken;
use tungstenite::{handshake::client::Request, Message};

mod auth;
pub mod bus;
mod common;
pub mod message;
pub mod resource;
pub mod server;
pub mod service;

pub type WebsocketConnection = WebSocketStream<TokioAdapter<TcpStream>>;

pub async fn connect(url: String) -> Result<WebsocketConnection, tungstenite::Error> {
    let tuple = connect_async(url).await?;
    Ok(tuple.0)
}

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

pub async fn bind(
    tcp: TcpStream,
    auth_token: WebsocketAuthToken,
) -> Result<WebsocketConnection, tungstenite::Error> {
    let auth = AuthHandler::new(auth_token);
    async_tungstenite::tokio::accept_hdr_async(tcp, auth).await
}

pub fn decode<T: DeserializeOwned>(
    message: Result<tungstenite::Message, tungstenite::Error>,
) -> anyhow::Result<T> {
    let message = message?;
    let data = bincode::deserialize::<T>(message.into_data().as_slice())?;
    Ok(data)
}

pub fn encode<T: Serialize>(message: T) -> anyhow::Result<tungstenite::Message> {
    let message = bincode::serialize(&message)?;
    Ok(Message::Binary(message))
}

pub fn encode_or_close<T: Serialize, F: FnOnce(&T) -> bool>(
    message: T,
    close_test: F,
) -> anyhow::Result<tungstenite::Message> {
    if close_test(&message) {
        return Ok(Message::Close(None));
    }

    let message = bincode::serialize(&message)?;
    Ok(Message::Binary(message))
}

pub fn encode_with<T: Serialize>(
    message: T,
) -> impl Future<Output = anyhow::Result<tungstenite::Message>> {
    futures::future::ready(encode(message))
}

pub fn decode_with<T: DeserializeOwned>(
    message: Result<tungstenite::Message, tungstenite::Error>,
) -> impl Future<Output = anyhow::Result<T>> {
    futures::future::ready(decode(message))
}
