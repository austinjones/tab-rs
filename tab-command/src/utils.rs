use lifeline::Receiver;
use postage::{stream::Stream, watch};
use thiserror::Error;

#[derive(Error, Debug)]
#[error("state never resolved to a value")]
pub struct StateUninitalizedError {}

pub fn state_or_default<T>(data: &mut Option<T>) -> &mut T
where
    T: Default,
{
    match data {
        Some(data) => data,
        None => {
            *data = Some(T::default());
            data.as_mut().unwrap()
        }
    }
}

#[allow(dead_code)]
pub async fn await_message<T: Clone, F, R>(
    channel: &mut impl Receiver<T>,
    mut condition: F,
) -> Result<R, StateUninitalizedError>
where
    F: FnMut(T) -> Option<R>,
{
    while let Some(message) = channel.recv().await {
        if let Some(ret) = condition(message) {
            return Ok(ret);
        }
    }

    Err(StateUninitalizedError {})
}

pub async fn await_state<T: Clone + Send + Sync>(
    channel: &mut watch::Receiver<Option<T>>,
) -> Result<T, StateUninitalizedError> {
    if let Some(ref value) = *channel.borrow() {
        return Ok(value.clone());
    }

    while let Some(update) = channel.recv().await {
        if let Some(value) = update {
            return Ok(value);
        }
    }

    Err(StateUninitalizedError {})
}

pub async fn await_condition<T: Clone, F>(
    channel: &mut watch::Receiver<Option<T>>,
    mut condition: F,
) -> Result<T, StateUninitalizedError>
where
    T: Clone + Send + Sync,
    F: FnMut(&T) -> bool,
{
    if let Some(ref value) = *channel.borrow() {
        if condition(value) {
            return Ok(value.clone());
        }
    }

    while let Some(update) = channel.recv().await {
        if let Some(value) = update {
            if condition(&value) {
                return Ok(value);
            }
        }
    }

    Err(StateUninitalizedError {})
}
