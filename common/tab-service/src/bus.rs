use dyn_clone::DynClone;
use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
    marker::PhantomData,
    ops::Deref,
    sync::RwLock,
};
use thiserror::Error;
use tokio::sync::mpsc;

pub trait Channel<Bus> {
    type Tx: Clone + 'static;
    type Rx: 'static;

    fn channel() -> (Self::Tx, Self::Rx);
}

pub trait Bus: Sized {
    /// Returns the receiver on the first call, and
    fn rx<Msg>(&self) -> Result<Msg::Rx, RxTakenError>
    where
        Msg: Channel<Self> + 'static;

    fn tx<Msg>(&self) -> Msg::Tx
    where
        Msg: Channel<Self> + 'static;
}

// struct Msg;

// struct MyChannelBus {}

// impl Channel<MyChannelBus> for Msg {
//     type Tx = mpsc::Sender<Msg>;
//     type Rx = mpsc::Receiver<Msg>;

//     fn channel() -> (Self::Tx, Self::Rx) {
//         mpsc::channel(1)
//     }
// }

#[derive(Error, Debug)]
#[error("receiver already taken")]
pub struct RxTakenError;
