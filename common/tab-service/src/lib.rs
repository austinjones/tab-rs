mod request;
// mod serialized_request;
mod spawn;

pub use request::Request;
pub use spawn::spawn;
// pub use spawn::spawn_from;
// pub use spawn::spawn_from_stream;
pub use spawn::Lifeline;

pub trait Service {
    type Rx;
    type Tx: Clone;

    fn spawn(rx: Self::Rx, tx: Self::Tx) -> Self;
    fn shutdown(self);
}
