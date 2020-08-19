mod request;
// mod serialized_request;
mod bus;
pub mod dyn_bus;
mod spawn;
pub mod tokio;

use async_trait::async_trait;

pub use request::Request;
pub use spawn::spawn;
// pub use spawn::spawn_from;
// pub use spawn::spawn_from_stream;
pub use bus::Bus;
pub use bus::Channel;
pub use bus::RxTakenError;
pub use spawn::Lifeline;

pub trait Service {
    type Tx: Clone;
    type Rx;

    fn spawn(rx: Self::Rx, tx: Self::Tx) -> Self;
}

#[async_trait]
pub trait AsyncService {
    type Tx: Clone;
    type Rx;

    async fn spawn(rx: Self::Rx, tx: Self::Tx) -> Self;
}
