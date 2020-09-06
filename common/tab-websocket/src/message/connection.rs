use tungstenite::Message as TungsteniteMessage;

/// A message received from the websocket, wraps tungstenite::Message
#[derive(Debug)]
pub struct WebsocketRecv(pub TungsteniteMessage);

/// A message sent over the websocket, wraps tungstenite::Message
#[derive(Debug)]
pub struct WebsocketSend(pub TungsteniteMessage);
