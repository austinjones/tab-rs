use std::{
    marker::PhantomData,
    sync::{Arc, RwLock},
};
use uuid::Uuid;

use lru::LruCache;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::sync::{oneshot, Mutex};

#[derive(Serialize, Deserialize, Debug)]
pub struct Request<Send, Recv>
where
    Send: Serialize + DeserializeOwned,
    Recv: Serialize + DeserializeOwned,
{
    #[serde(bound(deserialize = "Send: Serialize + DeserializeOwned"))]
    send: Send,
    recv: Uuid,
    _recv: PhantomData<Recv>,
}

pub struct Sender<Send, Recv> {
    internal: Arc<ReceiverInternal<Recv>>,
    _recv: PhantomData<Send>,
}

pub struct Receiver<Recv> {
    internal: Arc<ReceiverInternal<Recv>>,
}

struct ReceiverInternal<Recv> {
    channels: Mutex<LruCache<Uuid, oneshot::Sender<Recv>>>,
}

pub struct RequestSender {
    items: Arc<RwLock<LruCache<Uuid, oneshot::Sender<Recv>>>>,
}

impl<Recv> RequestService<Recv>
where
    Recv: Serialize + DeserializeOwned,
{
    pub fn send<Send>(&self, send: Send) -> (Request<Send, Recv>, oneshot::Receiver<Recv>)
    where
        Send: Serialize + DeserializeOwned,
    {
        let id = Uuid::new_v4();
        let request = Request::new(send, id);
        let (tx, rx) = oneshot::channel();

        {
            let read = self.items.read().await;
        }
    }

    pub fn notify(response: Response<Recv>) {}
}

impl<Send, Recv> Request<Send, Recv>
where
    Send: Serialize + DeserializeOwned,
    Recv: Serialize + DeserializeOwned,
{
    pub fn new(send: Send, uuid: Uuid) {}
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Response<Recv>
where
    Recv: Serialize + DeserializeOwned,
{
    #[serde(bound(deserialize = "Recv: Serialize + DeserializeOwned"))]
    recv: Recv,
}
