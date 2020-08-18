mod request;
// mod serialized_request;
mod spawn;

use async_trait::async_trait;

pub use request::Request;
pub use spawn::spawn;
// pub use spawn::spawn_from;
// pub use spawn::spawn_from_stream;
pub use spawn::Lifeline;

pub trait Service {
    type Rx;
    type Tx: Clone;

    fn spawn(rx: Self::Rx, tx: Self::Tx) -> Self;
    fn shutdown(self);
}

#[async_trait]
pub trait AsyncService {
    type Rx;
    type Tx: Clone;

    async fn spawn(rx: Self::Rx, tx: Self::Tx) -> Self;
    async fn shutdown(self);
}
