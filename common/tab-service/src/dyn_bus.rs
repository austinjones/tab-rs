use crate::{
    bus::{AlreadyLinkedError, Link, LinkTakenError, Message},
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

    pub fn get_tx<Chan>(&self) -> Option<&Chan::Tx>
    where
        Chan: Channel,
        Chan::Tx: Any + 'static,
    {
        self.value
            .as_ref()
            .map(|boxed| boxed.downcast_ref().unwrap())
    }

    pub fn clone_rx<Chan>(&mut self, tx: Option<&Chan::Tx>) -> Option<Chan::Rx>
    where
        Chan: Channel,
        Chan::Rx: Any + 'static,
    {
        let mut taken = self.value.take().map(Self::cast);
        let cloned = Chan::clone_rx(&mut taken, tx);
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
    state: RwLock<DynBusState>,
    _bus: PhantomData<B>,
}

#[derive(Debug)]
struct DynBusState {
    pub(crate) channels: HashSet<TypeId>,
    pub(crate) tx: HashMap<TypeId, BusSlot>,
    pub(crate) rx: HashMap<TypeId, BusSlot>,
    pub(crate) capacity: HashMap<TypeId, usize>,
}

impl Default for DynBusState {
    fn default() -> Self {
        DynBusState {
            channels: HashSet::new(),
            tx: HashMap::new(),
            rx: HashMap::new(),
            capacity: HashMap::new(),
        }
    }
}
impl<B: Bus> Default for DynBusStorage<B> {
    fn default() -> Self {
        DynBusStorage {
            state: RwLock::new(DynBusState::default()),
            _bus: PhantomData,
        }
    }
}

impl<B: Bus> DynBusStorage<B> {
    pub fn link_channel<Msg>(&self)
    where
        Msg: Message<B> + 'static,
    {
        let id = TypeId::of::<Msg>();

        if let Some(mut state) = self.try_lock(id) {
            let capacity = state
                .capacity
                .get(&id)
                .copied()
                .unwrap_or(Msg::Channel::default_capacity());

            let (tx, rx) = Msg::Channel::channel(capacity);

            state.rx.insert(id, BusSlot::new(rx));
            state.tx.insert(id, BusSlot::new(tx));

            state.channels.insert(id);
        }
    }

    pub fn clone_rx<Msg>(&self) -> Option<<Msg::Channel as Channel>::Rx>
    where
        Msg: Message<B> + 'static,
    {
        self.link_channel::<Msg>();

        let id = TypeId::of::<Msg>();

        let mut state = self.state.write().unwrap();
        let state = &mut *state;
        let tx = &state.tx;
        let rx = &mut state.rx;

        let tx = tx
            .get(&id)
            .expect("link_channel did not insert tx")
            .get_tx::<Msg::Channel>();

        let slot = rx.get_mut(&id).expect("link_channel did not insert rx");

        slot.clone_rx::<Msg::Channel>(tx)
    }

    pub fn clone_tx<Msg>(&self) -> Option<<Msg::Channel as Channel>::Tx>
    where
        Msg: Message<B> + 'static,
    {
        self.link_channel::<Msg>();

        let id = TypeId::of::<Msg>();

        let mut state = self.state.write().unwrap();
        let mut senders = &mut state.tx;
        let slot = senders
            .get_mut(&id)
            .expect("link_channel did not insert rx");

        slot.clone_tx::<Msg::Channel>()
    }

    pub fn capacity<Msg>(&self, capacity: usize) -> Result<(), AlreadyLinkedError>
    where
        Msg: Message<B> + 'static,
    {
        let id = TypeId::of::<Msg>();

        let state = self.state.read().unwrap();

        if state.capacity.contains_key(&id) {
            return Err(AlreadyLinkedError::new::<B, Msg>());
        }

        drop(state);

        let mut state = self.state.write().unwrap();

        state.capacity.insert(id, capacity);

        Ok(())
    }

    fn try_lock(&self, id: TypeId) -> Option<RwLockWriteGuard<DynBusState>> {
        let state = self.state.read().unwrap();
        if state.channels.contains(&id) {
            return None;
        }

        drop(state);

        let state = self.state.write().unwrap();
        if state.channels.contains(&id) {
            return None;
        }

        Some(state)
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
        self.storage()
            .clone_rx::<Msg>()
            .ok_or_else(|| LinkTakenError::new::<Self, Msg>(Link::Rx))
    }

    fn tx<Msg>(&self) -> Result<<Msg::Channel as Channel>::Tx, crate::bus::LinkTakenError>
    where
        Msg: crate::bus::Message<Self> + 'static,
    {
        self.storage().link_channel::<Msg>();
        self.storage()
            .clone_tx::<Msg>()
            .ok_or_else(|| LinkTakenError::new::<Self, Msg>(Link::Tx))
    }

    fn capacity<Msg>(&self, capacity: usize) -> Result<(), crate::bus::AlreadyLinkedError>
    where
        Msg: Message<Self> + 'static,
    {
        self.storage().capacity::<Msg>(capacity)
    }
}
