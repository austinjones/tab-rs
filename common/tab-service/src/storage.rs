pub trait Storage: Sized + 'static {
    /// If Self::Tx implements clone, clone it.  Otherwise use Option::take
    fn take_or_clone(res: &mut Option<Self>) -> Option<Self>;

    fn clone_slot(res: &mut Option<Self>) -> Option<Self>
    where
        Self: Clone,
    {
        res.as_ref().map(|t| t.clone())
    }

    fn take_slot(res: &mut Option<Self>) -> Option<Self> {
        res.take()
    }
}

#[macro_export]
macro_rules! impl_storage_take {
    ( $name:ty ) => {
        impl $crate::Storage for $name {
            fn take_or_clone(res: &mut Option<Self>) -> Option<Self> {
                Self::take_slot(res)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_channel_take {
    ( $name:ty ) => {
        impl<T: Send + 'static> $crate::Storage for $name {
            fn take_or_clone(res: &mut Option<Self>) -> Option<Self> {
                Self::take_slot(res)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_storage_clone {
    ( $name:ty ) => {
        impl $crate::Storage for $name {
            fn take_or_clone(res: &mut Option<Self>) -> Option<Self> {
                Self::clone_slot(res)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_channel_clone {
    ( $name:ty ) => {
        impl<T: Send + 'static> $crate::Storage for $name {
            fn take_or_clone(res: &mut Option<Self>) -> Option<Self> {
                Self::clone_slot(res)
            }
        }
    };
}
