use crate::type_name::type_name;
use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
    fmt::{Debug, Display},
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
    fn clone_rx(rx: &mut Option<Self::Rx>, tx: Option<&Self::Tx>) -> Option<Self::Rx>;
}

pub trait Message<Bus>: Debug {
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

#[derive(Debug)]
pub enum Link {
    Tx,
    Rx,
}

impl Display for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Link::Tx => f.write_str("Tx"),
            Link::Rx => f.write_str("Rx"),
        }
    }
}

//TODO: encode Bus and Link types
#[derive(Error, Debug)]
#[error("link already taken: {bus} < {message}::{link} >")]
pub struct LinkTakenError {
    pub bus: String,
    pub message: String,
    pub link: Link,
}

impl LinkTakenError {
    pub fn new<Bus, Message>(link: Link) -> Self {
        LinkTakenError {
            bus: type_name::<Bus>().to_string(),
            message: type_name::<Message>().to_string(),
            link,
        }
    }
}

#[derive(Error, Debug)]
#[error("link already generated: {bus} < {message} >")]
pub struct AlreadyLinkedError {
    pub bus: String,
    pub message: String,
}

impl AlreadyLinkedError {
    pub fn new<Bus, Message>() -> Self {
        AlreadyLinkedError {
            bus: type_name::<Bus>().to_string(),
            message: type_name::<Message>().to_string(),
        }
    }
}
