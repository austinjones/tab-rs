use crate::WebsocketConnection;
use futures::SinkExt;
use log::error;
use serde::Serialize;
use tungstenite::{Error, Message};

pub async fn send_message<Msg: Serialize>(websocket: &mut WebsocketConnection, message: Msg) {
    let encoded = bincode::serialize(&message);

    if let Err(e) = encoded {
        error!("failed to encode message: {}", e);
        return;
    }

    let sent = websocket.send(Message::Binary(encoded.unwrap())).await;
    if let Err(e) = sent {
        error!("failed to send message: {}", e);
    }
}

pub async fn send_close(websocket: &mut WebsocketConnection) {
    if let Err(e) = websocket.send(Message::Close(None)).await {
        match e {
            Error::ConnectionClosed | Error::AlreadyClosed | Error::Protocol(_) => {
                return;
            }
            _ => {
                error!("failed to send close frame: {}", e);
            }
        }
    }
}

pub fn should_terminate(message: &Message) -> bool {
    if let Message::Close(_) = message {
        true
    } else {
        false
    }
}
