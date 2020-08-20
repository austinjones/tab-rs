use crate::Channel;
use tokio::sync::{broadcast, mpsc, oneshot, watch};

pub(crate) fn clone<T: Clone>(link: &mut Option<T>) -> Option<T> {
    link.as_ref().map(|link| link.clone())
}

pub(crate) fn take<T>(link: &mut Option<T>) -> Option<T> {
    link.take()
}

impl<T: 'static> Channel for mpsc::Sender<T> {
    type Tx = Self;
    type Rx = mpsc::Receiver<T>;

    fn channel(capacity: usize) -> (Self::Tx, Self::Rx) {
        mpsc::channel(capacity)
    }

    fn default_capacity() -> usize {
        16
    }

    fn clone_tx(tx: &mut Option<Self::Tx>) -> Option<Self::Tx> {
        tx.as_ref().map(|tx| tx.clone())
    }

    fn clone_rx(rx: &mut Option<Self::Rx>, _: Option<&Self::Tx>) -> Option<Self::Rx> {
        rx.take()
    }
}

impl<T: 'static> Channel for broadcast::Sender<T> {
    type Tx = Self;
    type Rx = broadcast::Receiver<T>;

    fn channel(capacity: usize) -> (Self::Tx, Self::Rx) {
        broadcast::channel(capacity)
    }

    fn default_capacity() -> usize {
        16
    }

    fn clone_tx(tx: &mut Option<Self::Tx>) -> Option<Self::Tx> {
        clone(tx)
    }

    fn clone_rx(rx: &mut Option<Self::Rx>, tx: Option<&Self::Tx>) -> Option<Self::Rx> {
        tx.map(|tx| tx.subscribe())
    }
}

impl<T: 'static> Channel for oneshot::Sender<T> {
    type Tx = Self;
    type Rx = oneshot::Receiver<T>;

    fn channel(capacity: usize) -> (Self::Tx, Self::Rx) {
        oneshot::channel()
    }

    fn default_capacity() -> usize {
        1
    }

    fn clone_tx(tx: &mut Option<Self::Tx>) -> Option<Self::Tx> {
        take(tx)
    }

    fn clone_rx(rx: &mut Option<Self::Rx>, tx: Option<&Self::Tx>) -> Option<Self::Rx> {
        take(rx)
    }
}

impl<T> Channel for watch::Sender<T>
where
    T: Default + Clone + 'static,
{
    type Tx = Self;
    type Rx = watch::Receiver<T>;

    fn channel(capacity: usize) -> (Self::Tx, Self::Rx) {
        watch::channel(T::default())
    }

    fn default_capacity() -> usize {
        1
    }

    fn clone_tx(tx: &mut Option<Self::Tx>) -> Option<Self::Tx> {
        take(tx)
    }

    fn clone_rx(rx: &mut Option<Self::Rx>, tx: Option<&Self::Tx>) -> Option<Self::Rx> {
        take(rx)
    }
}
