use lifeline::Receiver;
use thiserror::Error;
use tokio::sync::watch;

#[derive(Error, Debug)]
#[error("state never resolved to a value")]
pub struct StateUninitalizedError {}

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

pub async fn await_state<T: Clone>(
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
    T: Clone,
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
