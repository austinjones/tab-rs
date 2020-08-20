use crate::{
    impl_channel_clone, impl_channel_take, Channel, Storage,
};
use tokio::sync::{broadcast, mpsc, oneshot, watch};

impl<T: 'static> Channel for mpsc::Sender<T> {
    type Tx = Self;
    type Rx = mpsc::Receiver<T>;

    fn channel(capacity: usize) -> (Self::Tx, Self::Rx) {
        mpsc::channel(capacity)
    }

    fn default_capacity() -> usize {
        16
    }
}

impl_channel_clone!(mpsc::Sender<T>);
impl_channel_take!(mpsc::Receiver<T>);

impl<T: 'static> Channel for broadcast::Sender<T> {
    type Tx = Self;
    type Rx = broadcast::Receiver<T>;

    fn channel(capacity: usize) -> (Self::Tx, Self::Rx) {
        broadcast::channel(capacity)
    }

    fn default_capacity() -> usize {
        16
    }

    fn clone_rx(rx: &mut Option<Self::Rx>, tx: Option<&Self::Tx>) -> Option<Self::Rx> {
        tx.map(|tx| tx.subscribe())
    }
}

impl_channel_take!(broadcast::Sender<T>);

// this is actually overriden in clone_rx
impl_channel_take!(broadcast::Receiver<T>);

impl<T: 'static> Channel for oneshot::Sender<T> {
    type Tx = Self;
    type Rx = oneshot::Receiver<T>;

    fn channel(_capacity: usize) -> (Self::Tx, Self::Rx) {
        oneshot::channel()
    }

    fn default_capacity() -> usize {
        1
    }
}

impl_channel_take!(oneshot::Sender<T>);
impl_channel_take!(oneshot::Receiver<T>);

impl<T> Channel for watch::Sender<T>
where
    T: Default + Clone + 'static,
{
    type Tx = Self;
    type Rx = watch::Receiver<T>;

    fn channel(_capacity: usize) -> (Self::Tx, Self::Rx) {
        watch::channel(T::default())
    }

    fn default_capacity() -> usize {
        1
    }
}

impl_channel_take!(watch::Sender<T>);
impl_channel_clone!(watch::Receiver<T>);
