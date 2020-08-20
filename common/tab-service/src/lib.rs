mod request;
// mod serialized_request;
mod bus;
pub mod dyn_bus;
mod spawn;
pub mod tokio;
mod type_name;

use async_trait::async_trait;
use futures::Future;
use spawn::spawn_task;
use std::fmt::Debug;

pub use request::Request;
// pub use spawn::spawn_from;
// pub use spawn::spawn_from_stream;
pub use bus::Bus;
pub use bus::Channel;
pub use bus::Message;
pub use spawn::Lifeline;

pub trait Service {
    type Rx;
    type Tx;
    type Lifeline;

    fn spawn(rx: Self::Rx, tx: Self::Tx) -> Self::Lifeline;

    fn task<Fut, Out>(name: &str, fut: Fut) -> Lifeline
    where
        Fut: Future<Output = Out> + Send + 'static,
        Out: Debug + Send + 'static,
        Self: Sized,
    {
        spawn_task::<Self, Fut, Out>(name, fut)
    }
}

#[async_trait]
pub trait AsyncService {
    type Rx;
    type Tx;
    type Lifeline;

    async fn spawn(rx: Self::Rx, tx: Self::Tx) -> Self::Lifeline;

    fn task<Fut, Out>(name: &str, fut: Fut) -> Lifeline
    where
        Fut: Future<Output = Out> + Send + 'static,
        Out: Debug + Send + 'static,
        Self: Sized,
    {
        spawn_task::<Self, Fut, Out>(name, fut)
    }
}
