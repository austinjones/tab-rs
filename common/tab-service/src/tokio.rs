use crate::Channel;
use tokio::sync::mpsc;

#[macro_export]
macro_rules! channel_tokio_mpsc {
    (impl Channel<$bus:ty, $capacity:literal> for $msg:ty) => {
        impl $crate::Channel<$bus> for $msg {
            type Rx = tokio::sync::mpsc::Receiver<$msg>;
            type Tx = tokio::sync::mpsc::Sender<$msg>;

            fn channel() -> (Self::Tx, Self::Rx) {
                tokio::sync::mpsc::channel($capacity)
            }
        }
    };
}
