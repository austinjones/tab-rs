use futures::{Future, Sink, SinkExt, Stream, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use std::borrow::Borrow;
use tungstenite::Message;

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
