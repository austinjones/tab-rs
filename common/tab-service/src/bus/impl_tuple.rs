use crate::{Bus, Channel};

macro_rules! impl_bus_tuple {
    ( ( $id:ident ),+ ) => {};
}

pub trait Busses {}
impl<A, B> Bus for (&A, &B)
where
    A: Bus,
    B: Bus,
{
    fn capacity<Msg>(&self, capacity: usize) -> Result<(), crate::AlreadyLinkedError>
    where
        Msg: crate::Message<Self> + 'static,
    {
        let r0 = self.0.capacity(capacity);
        let r1 = self.1.capacity(capacity);

        r0.and(r1)
    }
    fn rx<Msg>(&self) -> Result<<Msg::Channel as Channel>::Rx, crate::LinkTakenError>
    where
        Msg: crate::Message<Self> + 'static,
    {
        self.0.rx::<Msg>().or_else(|| self.0.rx::<Msg>())
    }
    fn tx<Msg>(&self) -> Result<<Msg::Channel as Channel>::Rx, crate::LinkTakenError>
    where
        Msg: crate::Message<Self> + 'static,
    {
        todo!()
    }
    fn resource<Res>(&self) -> Result<Res, crate::ResourceError>
    where
        Res: crate::Resource<Self>,
    {
        todo!()
    }
}
