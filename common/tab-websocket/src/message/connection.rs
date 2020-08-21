use tungstenite::Message as TungsteniteMessage;

#[derive(Debug)]
pub struct WebsocketRecv(pub TungsteniteMessage);

#[derive(Debug)]
pub struct WebsocketSend(pub TungsteniteMessage);
