use crate::{
    bus::{LinkTakenError, Message},
    Bus, Channel,
};
use dyn_clone::DynClone;
use impls::impls;
use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
    fmt::Debug,
    marker::PhantomData,
    ops::Deref,
    sync::{RwLock, RwLockWriteGuard},
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

pub(crate) struct BusSlot {
    value: Option<Box<dyn Any>>,
}

impl Debug for BusSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            Cloned => "BusSlot::Cloned(_)",
            Taken => "BusSlot::Taken(_)",
        };

        f.debug_struct(string).finish()
    }
}

impl BusSlot {
    pub fn new<T: 'static>(value: T) -> Self {
        Self {
            value: Some(Box::new(value)),
        }
    }

    pub fn clone_rx<Chan>(&mut self) -> Option<Chan::Rx>
    where
        Chan: Channel,
        Chan::Rx: Any + 'static,
    {
        let mut taken = self.value.take().map(Self::cast);
        let cloned = Chan::clone_rx(&mut taken);
        self.value = taken.map(|value| Box::new(value) as Box<dyn Any>);
        cloned
    }

    pub fn clone_tx<Chan>(&mut self) -> Option<Chan::Tx>
    where
        Chan: Channel,
        Chan::Tx: Any + 'static,
    {
        let mut taken = self.value.take().map(Self::cast);
        let cloned = Chan::clone_tx(&mut taken);
        self.value = taken.map(|value| Box::new(value) as Box<dyn Any>);
        cloned
    }

    // match self {
    //     Self::Taken(slot) => slot.take().map(|value| Self::cast(value)),
    //     Self::Cloned(ref boxed) => {
    //         let cloned = dyn_clone::clone_box(&**boxed);
    //         let any: Box<dyn Any> = Box::new(cloned);

    //         Some(Self::cast(any))
    //     }
    // }

    // // we can dynamically check if the type implements Clone!
    // if impls!(T: DynClone) {
    //     // if so, we downcast as a ref and clone
    //     if let Some(ref boxed) = self.data {
    //         // let value: T = *boxed
    //         //     .downcast::<T>()
    //         //     .expect("BusSlot should always have correct type");
    //         // let boxed: Box<&dyn AnyClone> = Box::new(&value);
    //         let boxed: Box<dyn DynClone> =
    //             traitcast::cast_box(boxed).expect("statically asserted");

    //         let cloned = dyn_clone::clone_box(&*boxed);

    //         let any: Box<dyn Any> = Box::new(cloned);

    //         let cloned = any
    //             .downcast::<T>()
    //             .expect("BusSlot should always have correct type");

    //         Some(*cloned)
    //     } else {
    //         None
    //     }
    // } else {
    //     // if not, we downcast as owned, and return
    //     self.data.take().map(|boxed| {})
    // }

    fn cast<T: 'static>(boxed: Box<dyn Any>) -> T {
        *boxed
            .downcast::<T>()
            .expect("BusSlot should always have correct type")
    }
}

#[derive(Debug)]
pub struct DynBusStorage<B> {
    pub(crate) channels: RwLock<HashSet<TypeId>>,
    pub(crate) tx: RwLock<HashMap<TypeId, BusSlot>>,
    pub(crate) rx: RwLock<HashMap<TypeId, BusSlot>>,

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
    pub fn try_lock(&self, id: TypeId) -> Option<RwLockWriteGuard<HashSet<TypeId>>> {
        let channels = self.channels.read().unwrap();
        if channels.contains(&id) {
            return None;
        }

        drop(channels);

        let channels = self.channels.write().unwrap();
        if channels.contains(&id) {
            return None;
        }

        Some(channels)
    }

    pub fn link_channel<Msg>(&self)
    where
        Msg: Message<B> + 'static,
    {
        let id = TypeId::of::<Msg>();

        if let Some(mut add_channel) = self.try_lock(id) {
            let (tx, rx) = Msg::Channel::channel();

            self.rx.write().unwrap().insert(id, BusSlot::new(rx));
            self.tx.write().unwrap().insert(id, BusSlot::new(tx));

            add_channel.insert(id);
        }
    }

    pub fn clone_rx<Msg>(&self) -> Option<<Msg::Channel as Channel>::Rx>
    where
        Msg: Message<B> + 'static,
    {
        self.link_channel::<Msg>();

        let id = TypeId::of::<Msg>();
        let mut receivers = self.rx.write().expect("lock posion");

        let slot = receivers
            .get_mut(&id)
            .expect("link_channel did not insert rx");

        slot.clone_rx::<Msg::Channel>()
    }

    pub fn clone_tx<Msg>(&self) -> Option<<Msg::Channel as Channel>::Tx>
    where
        Msg: Message<B> + 'static,
    {
        self.link_channel::<Msg>();

        let id = TypeId::of::<Msg>();
        let mut senders = self.tx.write().expect("lock posion");
        let slot = senders
            .get_mut(&id)
            .expect("link_channel did not insert rx");

        slot.clone_tx::<Msg::Channel>()
    }

    pub fn capacity<Msg>(&self, capacity: usize)
    where
        Msg: Message<B> + 'static,
    {
        let id = TypeId::of::<Msg>();

        if let Some(mut add_channel) = self.try_lock(id) {
            let (tx, rx) = Msg::Channel::channel();

            self.rx.write().unwrap().insert(id, BusSlot::new(rx));
            self.tx.write().unwrap().insert(id, BusSlot::new(tx));

            add_channel.insert(id);
        }
    }
}

impl<T> Bus for T
where
    T: DynBus,
{
    fn rx<Msg>(&self) -> Result<<Msg::Channel as Channel>::Rx, crate::bus::LinkTakenError>
    where
        Msg: crate::bus::Message<Self> + 'static,
    {
        self.storage().link_channel::<Msg>();
        self.storage().clone_rx::<Msg>().ok_or(LinkTakenError)
    }

    fn tx<Msg>(&self) -> Result<<Msg::Channel as Channel>::Tx, crate::bus::LinkTakenError>
    where
        Msg: crate::bus::Message<Self> + 'static,
    {
        self.storage().link_channel::<Msg>();
        self.storage().clone_tx::<Msg>().ok_or(LinkTakenError)
    }

    fn capacity<Msg>(&self, capacity: usize) -> Result<(), crate::bus::AlreadyLinkedError>
    where
        Msg: Message<Self> + 'static,
    {
        self.storage().capacity::<Msg>(capacity)
    }
}
