mod bus;
mod channels;
pub mod dyn_bus;
mod request;
mod spawn;
mod storage;
mod type_name;

use async_trait::async_trait;
use futures::Future;
use log::{debug, error};
use spawn::{spawn_task, task_name};
use std::{any::TypeId, fmt::Debug};
pub use storage::Storage;

pub use request::Request;
// pub use spawn::spawn_from;
// pub use spawn::spawn_from_stream;
pub use bus::*;
pub use spawn::Lifeline;

pub trait Service {
    type Bus: Bus;
    type Lifeline;

    fn spawn(bus: &Self::Bus) -> Self::Lifeline;

    fn task<Out>(name: &str, fut: impl Future<Output = Out> + Send + 'static) -> Lifeline
    where
        Out: Debug + Send + 'static,
        Self: Sized,
    {
        let service_name = task_name::<Self>(name);
        spawn_task(service_name, fut)
    }

    // TODO: anyhow feature
    fn try_task<Out>(
        name: &str,
        fut: impl Future<Output = anyhow::Result<Out>> + Send + 'static,
    ) -> Lifeline
    where
        Out: Debug + 'static,
        Self: Sized,
    {
        let service_name = task_name::<Self>(name);
        spawn_task(service_name.clone(), async move {
            match fut.await {
                Ok(val) => {
                    if TypeId::of::<Out>() != TypeId::of::<()>() {
                        debug!("OK {}, val: {:?}", service_name, val);
                    } else {
                        debug!("OK {}", service_name);
                    }
                }
                Err(e) => {
                    error!("ERR: {}, err: {}", service_name, e);
                }
            }
        })
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
        let service_name = task_name::<Self>(name);
        spawn_task(service_name, fut)
    }
}
