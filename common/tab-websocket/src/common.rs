use crate::WebsocketConnection;
use futures::SinkExt;
use log::error;
use tungstenite::{Error, Message};

/// Sends a close message over the provided websocket connection
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

/// Checks if, given the message, the connection should be closed.
pub fn should_terminate(message: &Message) -> bool {
    matches!(message, Message::Close(_))
}
