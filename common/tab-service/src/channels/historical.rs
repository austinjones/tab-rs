use crate::{Bus, Channel, Service};
pub use channel::{Receiver, Sender};
pub use messages::{Subscription, SubscriptionState};
use std::{fmt::Debug, hash::Hash};

// WIP historical channel - that preserves a history and provides each receiver a 'best effort' catch-up
pub trait ContentSize {
    fn size(&self) -> usize;
}

impl<T> Channel for channel::Sender<T>
where
    T: ContentSize + Clone + Debug + Send + Sync + 'static,
{
    type Tx = channel::Sender<T>;
    type Rx = channel::Receiver<T>;

    fn channel(capacity: usize) -> (Self::Tx, Self::Rx) {
        channel(capacity)
    }

    fn default_capacity() -> usize {
        32
    }
}

pub fn channel<T>(capacity: usize) -> (channel::Sender<T>, channel::Receiver<T>) {
    let sender = channel::Sender::new(capacity);
    let receiver = sender.subscribe();

    (sender, receiver)
}

mod bus {
    use crate::{service_bus, Message};
    use std::{fmt::Debug, hash::Hash};
    use tokio::sync::{mpsc, watch};

    service_bus!(pub HistoricalBus<T>);

    impl<T> Message<HistoricalBus<T>> for super::messages::Input<T>
    where
        T: Debug + Send + Sync + 'static,
    {
        type Channel = mpsc::Sender<Self>;
    }
}

mod channel {
    use super::messages::History;
    use crate::{impl_channel_clone, Lifeline};
    use std::{collections::VecDeque, hash::Hash, sync::Arc};
    use tokio::sync::{mpsc, watch};

    pub struct State<T> {
        state: VecDeque<History<T>>,
    }

    impl<T> State<T> {
        pub fn next(&self, index: usize) {
            if let Some(front) = self.state.front() {}
        }
    }

    #[derive(Debug)]
    pub struct Sender<T> {
        tx: mpsc::Sender<Subscription<T>>,
        // TODO: store in sender and receiver.
        // in-flight messages should still be processed, even if the sender disconneccts
        state: Arc<State>,
    }

    impl<T> Sender<T> {
        pub(crate) fn new(tx: mpsc::Sender<Subscription<T>>, update: Lifeline) -> Self {
            Self {
                tx,
                _update: Arc::new(update),
            }
        }

        pub async fn send(
            &mut self,
            subscription: Subscription<T>,
        ) -> Result<(), mpsc::error::SendError<Subscription<T>>> {
            self.tx.send(subscription).await
        }

        pub fn try_send(
            &mut self,
            subscription: Subscription<T>,
        ) -> Result<(), mpsc::error::TrySendError<Subscription<T>>> {
            self.tx.try_send(subscription)
        }
    }

    impl<T> Clone for Sender<T> {
        fn clone(&self) -> Self {
            Self {
                tx: self.tx.clone(),
                _update: self._update.clone(),
            }
        }
    }

    impl_channel_clone!(Sender<T>);
    // impl<T: Send + 'static> crate::Storage for Sender<T> {
    //     fn take_or_clone(res: &mut Option<Self>) -> Option<Self> {
    //         Self::clone_slot(res)
    //     }
    // }

    #[derive(Debug)]
    pub struct Receiver<T> {
        rx: watch::Receiver<SubscriptionState<T>>,
    }

    impl<T> Receiver<T> {
        pub fn new(rx: watch::Receiver<SubscriptionState<T>>) -> Self {
            Self { rx }
        }
    }

    impl<T: Hash + Eq> Receiver<T> {
        pub fn contains(&self, id: &T) -> bool {
            self.rx.borrow().contains(id)
        }

        pub fn get_identifier(&self, id: &T) -> Option<usize> {
            self.rx.borrow().get(id)
        }
    }

    impl<T> Clone for Receiver<T> {
        fn clone(&self) -> Self {
            Self {
                rx: self.rx.clone(),
            }
        }
    }

    impl_channel_clone!(Receiver<T>);
}

mod messages {
    use std::{
        collections::{HashMap, HashSet},
        hash::Hash,
    };

    #[derive(Debug, Clone)]
    pub struct History<T> {
        pub index: usize,
        pub item: T,
    }
}

mod service {
    use super::messages::History;
    use crate::{Bus, Lifeline, Service};
    use std::{fmt::Debug, hash::Hash, marker::PhantomData};

    pub struct UpdateService<T> {
        _t: PhantomData<T>,
    }

    impl<T> Service for UpdateService<T>
    where
        T: Clone + Hash + Eq + Debug + Send + Sync + 'static,
    {
        type Bus = super::bus::HistoricalBus<T>;
        type Lifeline = anyhow::Result<Lifeline>;

        fn spawn(bus: &Self::Bus) -> Self::Lifeline {
            let mut rx = bus.rx::<History<T>>()?;
            let tx = bus.tx::<SubscriptionState<T>>()?;
            let mut next_id = 0usize;
            let lifeline = Self::try_task("run", async move {
                let mut state = SubscriptionState::default();
                while let Some(msg) = rx.recv().await {
                    match msg {
                        Subscription::Subscribe(id) => {
                            if state.subscriptions.contains_key(&id) {
                                continue;
                            }

                            state.subscriptions.insert(id, next_id);
                            tx.broadcast(state.clone())?;
                            next_id += 1;
                        }
                        Subscription::Unsubscribe(id) => {
                            if !state.subscriptions.contains_key(&id) {
                                continue;
                            }

                            state.subscriptions.remove(&id);
                            tx.broadcast(state.clone())?;
                        }
                    }
                }

                Ok(())
            });

            Ok(lifeline)
        }
    }
}
