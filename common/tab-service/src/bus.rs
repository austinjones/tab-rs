use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
    marker::PhantomData,
    ops::Deref,
    sync::RwLock,
};
use thiserror::Error;
use tokio::sync::mpsc;

pub trait Channel {
    type Tx: 'static;
    type Rx: 'static;

    fn channel(capacity: usize) -> (Self::Tx, Self::Rx);

    fn default_capacity() -> usize;

    /// If Self::Tx implements clone, clone it.  Otherwise use Option::take
    fn clone_tx(tx: &mut Option<Self::Tx>) -> Option<Self::Tx>;

    /// If Self::Tx implements clone, clone it.  Otherwise use Option::take
    fn clone_rx(rx: &mut Option<Self::Rx>) -> Option<Self::Rx>;
}

pub trait Message<Bus> {
    type Channel: Channel;
}

pub trait Bus: Sized {
    /// Returns the receiver on the first call, and

    fn capacity<Msg>(&self, capacity: usize) -> Result<(), AlreadyLinkedError>
    where
        Msg: Message<Self> + 'static;

    fn rx<Msg>(&self) -> Result<<Msg::Channel as Channel>::Rx, LinkTakenError>
    where
        Msg: Message<Self> + 'static;

    fn tx<Msg>(&self) -> Result<<Msg::Channel as Channel>::Tx, LinkTakenError>
    where
        Msg: Message<Self> + 'static;
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
#[error("rx/tx link already taken")]
pub struct LinkTakenError;

#[derive(Error, Debug)]
#[error("link already generated - capacity is immutable")]
pub struct AlreadyLinkedError;
