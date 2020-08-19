use crate::{Bus, Channel, RxTakenError};
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

#[macro_export]
macro_rules! service_bus (
    (pub $name:ident) => {
        #[derive(Debug, Default)]
        pub struct $name {
            storage: $crate::dyn_bus::DynBusStorage<Self>,
        }

        impl $crate::dyn_bus::DynBus for $name {
            fn storage(&self) -> &$crate::dyn_bus::DynBusStorage<Self> {
                &self.storage
            }
        }
    };

    ($name:ident) => {
        #[derive(Debug, Default)]
        struct $name {
            storage: $crate::dyn_bus::DynBusStorage<Self>,
        }

        impl $crate::dyn_bus::DynBus for $name {
            fn storage(&self) -> &$crate::dyn_bus::DynBusStorage<Self> {
                &self.storage
            }
        }
    };
);

pub trait DynBus: Bus {
    fn storage(&self) -> &DynBusStorage<Self>;
}

#[derive(Debug)]
pub struct DynBusStorage<B> {
    pub(super) channels: RwLock<HashSet<TypeId>>,
    pub(super) tx: RwLock<HashMap<TypeId, Box<dyn Any>>>,
    pub(super) rx: RwLock<HashMap<TypeId, Option<Box<dyn Any>>>>,

    _bus: PhantomData<B>,
}

impl<B: Bus> Default for DynBusStorage<B> {
    fn default() -> Self {
        DynBusStorage {
            channels: RwLock::new(HashSet::new()),
            tx: RwLock::new(HashMap::new()),
            rx: RwLock::new(HashMap::new()),
            _bus: PhantomData,
        }
    }
}

impl<B: Bus> DynBusStorage<B> {
    pub fn link_channel<Msg>(&self)
    where
        Msg: Channel<B> + 'static,
    {
        let id = TypeId::of::<Msg>();

        let channels = self.channels.read().unwrap();
        if channels.contains(&id) {
            return;
        }

        drop(channels);

        let mut channels = self.channels.write().unwrap();
        if channels.contains(&id) {
            return;
        }

        let (tx, rx) = Msg::channel();

        self.rx.write().unwrap().insert(id, Some(Box::new(rx)));

        self.tx.write().unwrap().insert(id, Box::new(tx));

        channels.insert(id);
    }

    pub fn take_rx<Msg>(&self) -> Option<Msg::Rx>
    where
        Msg: Channel<B> + 'static,
    {
        self.link_channel::<Msg>();

        let id = TypeId::of::<Msg>();
        let mut receivers = self.rx.write().expect("lock posion");

        let slot = receivers
            .get_mut(&id)
            .expect("link_channel did not insert rx");

        slot.take().map(|boxed| {
            *boxed
                .downcast::<Msg::Rx>()
                .expect("DynBusStorage should always have correct Rx type")
        })
    }

    pub fn clone_tx<Msg>(&self) -> Msg::Tx
    where
        Msg: Channel<B> + 'static,
    {
        self.link_channel::<Msg>();

        let id = TypeId::of::<Msg>();
        let senders = self.tx.read().expect("lock posion");
        let boxed = senders.get(&id).expect("link_channel did not insert rx");

        boxed
            .downcast_ref::<Msg::Tx>()
            .expect("DynBusStorage should always have correct Tx type")
            .clone()
    }
}

impl<T> Bus for T
where
    T: DynBus,
{
    fn rx<Msg>(&self) -> Result<Msg::Rx, RxTakenError>
    where
        Msg: Channel<Self> + 'static,
    {
        self.storage().take_rx::<Msg>().ok_or(RxTakenError)
    }

    fn tx<Msg>(&self) -> Msg::Tx
    where
        Msg: Channel<Self> + 'static,
    {
        self.storage().clone_tx::<Msg>()
    }
}
