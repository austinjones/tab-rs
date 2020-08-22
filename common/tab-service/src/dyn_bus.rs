use crate::{
    bus::{AlreadyLinkedError, Link, Message, Resource, TakeResourceError},
    type_name::type_name,
    Bus, Channel, Storage, TakeChannelError,
};

use log::debug;
use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
    fmt::Debug,
    marker::PhantomData,
    sync::{RwLock, RwLockWriteGuard},
};

#[macro_export]
macro_rules! service_bus (
            // match one or more generics separated by a comma
    ($name:ident $(< $( $gen:ident ),+ >)? ) => {
        service_bus! { () struct $name $(< $( $gen ),+ >)? }
    };

    (pub $name:ident $(< $( $gen:ident ),+ >)* ) => {
        service_bus! { (pub) struct $name $(< $( $gen ),+ >)* }
    };

    (($($vis:tt)*) struct $name:ident $(< $( $gen:ident ),+ >)? ) => {
        #[derive(Debug)]
        #[allow(non_snake_case)]
        $($vis)* struct $name $(< $( $gen: std::fmt::Debug ),+ >)? {
            storage: $crate::dyn_bus::DynBusStorage<Self>,
            $(
                $( $gen: std::marker::PhantomData<$gen> ),+
            )?
        }

        impl$(< $( $gen: std::fmt::Debug ),+ >)? std::default::Default for $name $(< $( $gen ),+ >)? {
            fn default() -> Self {
                Self {
                    storage: $crate::dyn_bus::DynBusStorage::default(),
                    $(
                        $( $gen: std::marker::PhantomData::<$gen> ),+
                    )?
                }
            }
        }

        impl$(< $( $gen: std::fmt::Debug ),+ >)? $crate::dyn_bus::DynBus for $name$(< $( $gen ),+ >)? {
            fn store_rx<Msg>(&self, rx: <Msg::Channel as $crate::Channel>::Rx) -> Result<(), $crate::AlreadyLinkedError>
                where Msg: $crate::Message<Self> + 'static
            {
                self.storage().store_channel::<Msg, Msg::Channel, Self>(Some(rx), None)
            }

            fn store_tx<Msg>(&self, tx: <Msg::Channel as $crate::Channel>::Tx) -> Result<(), $crate::AlreadyLinkedError>
                where Msg: $crate::Message<Self> + 'static
            {
                self.storage().store_channel::<Msg, Msg::Channel, Self>(None, Some(tx))
            }

            fn store_channel<Msg>(
                &self,
                rx: <Msg::Channel as $crate::Channel>::Rx,
                tx: <Msg::Channel as $crate::Channel>::Tx
            ) -> Result<(), $crate::AlreadyLinkedError>
                where Msg: $crate::Message<Self> + 'static
            {
                self.storage().store_channel::<Msg, Msg::Channel, Self>(Some(rx), Some(tx))
            }

            fn store_resource<R: $crate::Resource<Self>>(&self, resource: R) {
                self.storage.store_resource::<R, Self>(resource)
            }

            fn take_channel<Msg, Source>(
                &self,
                other: &Source,
            ) -> Result<(), $crate::TakeChannelError>
            where
                Msg: $crate::Message<Self> + 'static,
                Msg: $crate::Message<Source, Channel = <Msg as $crate::Message<Self>>::Channel>,
                Source: $crate::dyn_bus::DynBus
            {
                self.storage.take_channel::<Msg, Source, Self, <Msg as $crate::Message<Self>>::Channel>(other, true, true)
            }

            fn take_rx<Msg, Source>(
                &self,
                other: &Source,
            ) -> Result<(), $crate::TakeChannelError>
            where
                Msg: $crate::Message<Self> + 'static,
                Msg: $crate::Message<Source, Channel = <Msg as $crate::Message<Self>>::Channel>,
                Source: $crate::dyn_bus::DynBus
            {
                self.storage.take_channel::<Msg, Source, Self, <Msg as $crate::Message<Self>>::Channel>(other, true, false)
            }

            fn take_tx<Msg, Source>(
                &self,
                other: &Source,
            ) -> Result<(), $crate::TakeChannelError>
            where
                Msg: $crate::Message<Self> + 'static,
                Msg: $crate::Message<Source, Channel = <Msg as $crate::Message<Self>>::Channel>,
                Source: $crate::dyn_bus::DynBus
            {
                self.storage.take_channel::<Msg, Source, Self, <Msg as $crate::Message<Self>>::Channel>(other, false, true)
            }

            fn take_resource<Res, Source>(
                &self,
                other: &Source,
            ) -> Result<(), $crate::TakeResourceError>
            where
                Res: $crate::Storage,
                Res: $crate::Resource<Source>,
                Res: $crate::Resource<Self>,
                Source: $crate::dyn_bus::DynBus
            {
                self.storage.take_resource::<Res, Source, Self>(other)
            }

            fn storage(&self) -> &$crate::dyn_bus::DynBusStorage<Self> {
                &self.storage
            }
        }
    }
    // ($name:ident) => {
    //     #[derive(Debug, Default)]
    //     struct $name {
    //         storage: $crate::dyn_bus::DynBusStorage<Self>,
    //     }

    //     impl $crate::dyn_bus::DynBus for $name {
    //         fn storage(&self) -> &$crate::dyn_bus::DynBusStorage<Self> {
    //             &self.storage
    //         }
    //     }
    // };
);

pub trait DynBus: Bus {
    fn store_rx<Msg>(&self, rx: <Msg::Channel as Channel>::Rx) -> Result<(), AlreadyLinkedError>
    where
        Msg: Message<Self> + 'static;

    fn store_tx<Msg>(&self, tx: <Msg::Channel as Channel>::Tx) -> Result<(), AlreadyLinkedError>
    where
        Msg: Message<Self> + 'static;

    fn store_channel<Msg>(
        &self,
        rx: <Msg::Channel as Channel>::Rx,
        tx: <Msg::Channel as Channel>::Tx,
    ) -> Result<(), AlreadyLinkedError>
    where
        Msg: Message<Self> + 'static;

    fn store_resource<R: Resource<Self>>(&self, resource: R);

    fn take_channel<Msg, Source>(&self, other: &Source) -> Result<(), TakeChannelError>
    where
        Msg: Message<Self> + 'static,
        Msg: Message<Source, Channel = <Msg as Message<Self>>::Channel>,
        Source: DynBus;

    fn take_rx<Msg, Source>(&self, other: &Source) -> Result<(), TakeChannelError>
    where
        Msg: Message<Self> + 'static,
        Msg: Message<Source, Channel = <Msg as Message<Self>>::Channel>,
        Source: DynBus;

    fn take_tx<Msg, Source>(&self, other: &Source) -> Result<(), TakeChannelError>
    where
        Msg: Message<Self> + 'static,
        Msg: Message<Source, Channel = <Msg as Message<Self>>::Channel>,
        Source: DynBus;

    fn take_resource<Res, Source>(&self, other: &Source) -> Result<(), TakeResourceError>
    where
        Res: Storage,
        Res: Resource<Source>,
        Res: Resource<Self>,
        Source: DynBus;

    fn storage(&self) -> &DynBusStorage<Self>;
}

pub(crate) struct BusSlot {
    name: String,
    value: Option<Box<dyn Any + Send>>,
}

impl Debug for BusSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self.value {
            Some(_) => format!("BusSlot<{}>::Some(_)", self.name.as_str()),
            None => format!("BusSlot<{}>::Empty", self.name.as_str()),
        };

        f.debug_struct(string.as_str()).finish()
    }
}

impl BusSlot {
    pub fn new<T: Send + 'static>(value: Option<T>) -> Self {
        Self {
            // TODO: think about this?  uses memory, but it's nice for debugging
            name: type_name::<T>(),
            value: value.map(|v| Box::new(v) as Box<dyn Any + Send>),
        }
    }

    pub fn empty<T>() -> Self {
        Self {
            name: type_name::<T>(),
            value: None,
        }
    }

    pub fn put<T: Send + 'static>(&mut self, value: T) {
        self.value = Some(Box::new(value))
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
        Chan::Rx: Storage + Send + 'static,
    {
        let mut taken = self.value.take().map(Self::cast);
        let cloned = Chan::clone_rx(&mut taken, tx);
        self.value = taken.map(|value| Box::new(value) as Box<dyn Any + Send>);
        cloned
    }

    pub fn clone_tx<Chan>(&mut self) -> Option<Chan::Tx>
    where
        Chan: Channel,
        Chan::Tx: Storage + Send + 'static,
    {
        let mut taken = self.value.take().map(Self::cast);
        let cloned = Chan::clone_tx(&mut taken);
        self.value = taken.map(|value| Box::new(value) as Box<dyn Any + Send>);
        cloned
    }

    pub fn clone_storage<Res>(&mut self) -> Option<Res>
    where
        Res: Storage + Send + Any,
    {
        let mut taken = self.value.take().map(Self::cast);
        let cloned = Res::take_or_clone(&mut taken);
        self.value = taken.map(|value| Box::new(value) as Box<dyn Any + Send>);
        cloned
    }

    fn cast<T: 'static>(boxed: Box<dyn Any + Send>) -> T {
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
    pub(crate) capacity: HashMap<TypeId, usize>,
    pub(crate) tx: HashMap<TypeId, BusSlot>,
    pub(crate) rx: HashMap<TypeId, BusSlot>,
    pub(crate) resources: HashMap<TypeId, BusSlot>,
}

impl Default for DynBusState {
    fn default() -> Self {
        DynBusState {
            channels: HashSet::new(),
            capacity: HashMap::new(),
            tx: HashMap::new(),
            rx: HashMap::new(),
            resources: HashMap::new(),
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
    pub fn link_channel<Msg, Bus>(&self)
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

            debug!("{} linked in {}", type_name::<Msg>(), type_name::<Bus>());
            state.rx.insert(id, BusSlot::new(Some(rx)));
            state.tx.insert(id, BusSlot::new(Some(tx)));

            state.channels.insert(id);
        }
    }

    pub fn clone_rx<Msg, Bus>(&self) -> Result<<Msg::Channel as Channel>::Rx, TakeChannelError>
    where
        Msg: Message<B> + 'static,
    {
        self.link_channel::<Msg, Bus>();

        let id = TypeId::of::<Msg>();

        let mut state = self.state.write().unwrap();
        let state = &mut *state;
        let tx = &state.tx;
        let rx = &mut state.rx;

        let tx = tx
            .get(&id)
            .map(|slot| slot.get_tx::<Msg::Channel>())
            .flatten();

        let slot = rx
            .get_mut(&id)
            .ok_or_else(|| TakeChannelError::partial_take::<Bus, Msg>(Link::Rx))?;

        slot.clone_rx::<Msg::Channel>(tx)
            .ok_or_else(|| TakeChannelError::already_taken::<Bus, Msg>(Link::Tx))
    }

    pub fn clone_tx<Msg, Bus>(&self) -> Result<<Msg::Channel as Channel>::Tx, TakeChannelError>
    where
        Msg: Message<B> + 'static,
    {
        self.link_channel::<Msg, Bus>();

        let id = TypeId::of::<Msg>();

        let mut state = self.state.write().unwrap();
        let senders = &mut state.tx;

        // if the channel is linked, but the slot is empty,
        // this means the user used take_rx, but asked for tx
        let slot = senders
            .get_mut(&id)
            .ok_or_else(|| TakeChannelError::partial_take::<Bus, Msg>(Link::Tx))?;

        slot.clone_tx::<Msg::Channel>()
            .ok_or_else(|| TakeChannelError::already_taken::<Bus, Msg>(Link::Tx))
    }

    pub fn clone_resource<Res>(&self) -> Result<Res, TakeResourceError>
    where
        Res: Resource<B> + 'static,
    {
        let id = TypeId::of::<Res>();

        let mut state = self.state.write().unwrap();
        let resources = &mut state.resources;
        let slot = resources
            .get_mut(&id)
            .ok_or_else(|| TakeResourceError::uninitialized::<Self, Res>())?;

        slot.clone_storage::<Res>()
            .ok_or_else(|| TakeResourceError::taken::<Self, Res>())
    }

    pub fn store_resource<Res: Send + 'static, Bus>(&self, value: Res) {
        let id = TypeId::of::<Res>();

        let mut state = self.state.write().unwrap();
        let resources = &mut state.resources;

        if !resources.contains_key(&id) {
            resources.insert(id.clone(), BusSlot::empty::<Res>());
        }

        debug!("{} stored in {}", type_name::<Res>(), type_name::<Bus>());

        let slot = resources.get_mut(&id).unwrap();

        slot.put(value);
    }

    pub fn store_channel<Msg, Chan, Bus>(
        &self,
        rx: Option<Chan::Rx>,
        tx: Option<Chan::Tx>,
    ) -> Result<(), AlreadyLinkedError>
    where
        Chan: Channel,
        Msg: 'static,
    {
        if rx.is_none() && tx.is_none() {
            return Ok(());
        }

        let id = TypeId::of::<Msg>();

        let mut target = self.state.write().expect("cannot lock other");
        if target.channels.contains(&id) {
            return Err(AlreadyLinkedError::new::<Self, Msg>());
        }

        let link = match (rx.is_some(), tx.is_some()) {
            (true, true) => Link::Both,
            (true, false) => Link::Rx,
            (false, true) => Link::Tx,
            (false, false) => unreachable!(),
        };

        debug!(
            "{}/{} stored in {}",
            type_name::<Msg>(),
            link,
            type_name::<Bus>(),
        );

        target.channels.insert(id);
        target.tx.insert(id.clone(), BusSlot::new(tx));
        target.rx.insert(id.clone(), BusSlot::new(rx));

        Ok(())
    }

    pub fn take_channel<Msg, Source, Target, Chan>(
        &self,
        other: &Source,
        rx: bool,
        tx: bool,
    ) -> Result<(), crate::bus::TakeChannelError>
    where
        Msg: Message<Target, Channel = Chan> + Message<Source, Channel = Chan> + 'static,
        Chan: Channel,
        Source: DynBus,
    {
        // TODO: clean up this function.  way too complicated
        if !rx && !tx {
            return Ok(());
        }

        other.storage().link_channel::<Msg, Source>();

        let id = TypeId::of::<Msg>();

        let mut target = self.state.write().expect("cannot lock other");
        if target.channels.contains(&id) {
            return Err(TakeChannelError::already_linked::<Target, Msg>());
        }

        let (rx_value, tx_value) = {
            let source = other.storage();
            let mut source = source.state.write().expect("cannot lock source");

            let tx_value = if tx {
                source
                    .tx
                    .get_mut(&id)
                    .map(|v| v.clone_tx::<Chan>())
                    .flatten()
            } else {
                None
            };

            let rx_value = if rx {
                source
                    .rx
                    .get_mut(&id)
                    .map(|v| v.clone_rx::<Chan>(tx_value.as_ref()))
                    .flatten()
            } else {
                None
            };

            (rx_value, tx_value)
        };

        let rx_missing = rx && rx_value.is_none();
        let tx_missing = tx && tx_value.is_none();
        match (rx_missing, tx_missing) {
            (true, true) => {
                return Err(TakeChannelError::already_taken::<Source, Msg>(Link::Both));
            }
            (true, false) => {
                return Err(TakeChannelError::already_taken::<Source, Msg>(Link::Rx));
            }
            (false, true) => {
                return Err(TakeChannelError::already_taken::<Source, Msg>(Link::Tx));
            }
            _ => {}
        }

        let link = match (rx && rx_value.is_some(), tx && tx_value.is_some()) {
            (true, true) => Link::Both,
            (true, false) => Link::Rx,
            (false, true) => Link::Tx,
            (false, false) => unreachable!(),
        };

        target.channels.insert(id);
        debug!(
            "{}/{} moved: {} => {}",
            type_name::<Msg>(),
            link,
            type_name::<Source>(),
            type_name::<Target>()
        );

        if rx {
            target.rx.insert(id.clone(), BusSlot::new(rx_value));
        }

        if tx {
            target.tx.insert(id.clone(), BusSlot::new(tx_value));
        }

        Ok(())
    }

    pub fn take_resource<Res, Source, Target>(
        &self,
        other: &Source,
    ) -> Result<(), crate::bus::TakeResourceError>
    where
        Res: Resource<Source>,
        Res: Resource<Target>,
        Res: Storage,
        Source: DynBus,
    {
        let id = TypeId::of::<Res>();

        let mut target = self.state.write().expect("cannot lock other");
        if target.resources.contains_key(&id) {
            return Err(TakeResourceError::taken::<Source, Res>());
        }

        let source = other.storage();

        let res = source.clone_resource::<Res>()?;
        drop(source);

        debug!(
            "Resource {} moved: {} => {}",
            type_name::<Res>(),
            type_name::<Source>(),
            type_name::<Target>()
        );
        target.resources.insert(id.clone(), BusSlot::new(Some(res)));

        Ok(())
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
    fn rx<Msg>(&self) -> Result<<Msg::Channel as Channel>::Rx, crate::bus::TakeChannelError>
    where
        Msg: crate::bus::Message<Self> + 'static,
    {
        self.storage().link_channel::<Msg, Self>();
        self.storage().clone_rx::<Msg, Self>()
    }

    fn tx<Msg>(&self) -> Result<<Msg::Channel as Channel>::Tx, crate::bus::TakeChannelError>
    where
        Msg: crate::bus::Message<Self> + 'static,
    {
        self.storage().link_channel::<Msg, Self>();
        self.storage().clone_tx::<Msg, Self>()
    }

    fn capacity<Msg>(&self, capacity: usize) -> Result<(), crate::bus::AlreadyLinkedError>
    where
        Msg: Message<Self> + 'static,
    {
        self.storage().capacity::<Msg>(capacity)
    }

    fn resource<Res>(&self) -> Result<Res, TakeResourceError>
    where
        Res: Resource<Self>,
    {
        self.storage().clone_resource::<Res>()
    }
}
