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
pub use spawn::Lifeline;

pub trait Service {
    type Rx;
    type Tx;
    type Return;

    fn spawn(rx: Self::Rx, tx: Self::Tx) -> Self::Return;
}

#[async_trait]
pub trait AsyncService {
    type Rx;
    type Tx;
    type Return;

    async fn spawn(rx: Self::Rx, tx: Self::Tx) -> Self::Return;
}
