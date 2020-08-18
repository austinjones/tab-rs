use futures::Future;
use tokio::sync::oneshot;

pub struct Request<Send, Recv> {
    send: Send,
    recv: oneshot::Sender<Recv>,
}

impl<Send, Recv> Request<Send, Recv> {
    /// Constructs a pair of Request, and Receiver for the response
    pub async fn send(send: Send) -> (Self, oneshot::Receiver<Recv>) {
        let (tx, rx) = oneshot::channel();
        let request = Self { send, recv: tx };
        (request, rx)
    }

    /// Asynchronously replies to the given request, using the provided closure
    pub async fn reply<Fn, Fut>(self, respond: Fn) -> Result<(), Recv>
    where
        Fn: FnOnce(Send) -> Fut,
        Fut: Future<Output = Recv>,
    {
        let response = respond(self.send).await;
        self.recv.send(response)
    }
}
