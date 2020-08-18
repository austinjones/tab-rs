use futures::{task::AtomicWaker, Future, Stream};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::Poll,
};

use pin_project::pin_project;
use tokio::stream::StreamExt;

/// Executes the task, until the future completes, or the lifeline is dropped
pub fn spawn<O: Send + 'static, F: Future<Output = O> + Send + 'static>(fut: F) -> Lifeline {
    let inner = Arc::new(LifelineInner::new());

    let service = ServiceFuture::new(fut, inner.clone());
    spawn_task(service);

    Lifeline::new(inner)
}

pub fn spawn_from_stream<T, S, F, Fut>(mut stream: S, mut f: F) -> Lifeline
where
    S: Stream<Item = T> + Send + Unpin + 'static,
    F: FnMut(T) -> Fut + Send + Sync + 'static,
    T: Send,
    Fut: Future + Send,
{
    let future = async move {
        while let Some(msg) = stream.next().await {
            f(msg).await;
        }
    };

    spawn(future)
}

pub fn spawn_from<State, Output, Fut, F>(state: State, function: F) -> Lifeline
where
    Output: Send + 'static,
    Fut: Future<Output = Output> + Send + 'static,
    F: FnOnce(State) -> Fut,
{
    let future = function(state);
    spawn(future)
}

// #[cfg(feature = "tokio-executor")]
fn spawn_task<O: Send + 'static, F: Future<Output = O> + Send + 'static>(task: F) {
    tokio::spawn(task);
}

#[pin_project]
struct ServiceFuture<F: Future> {
    #[pin]
    future: F,

    inner: Arc<LifelineInner>,
}

impl<F: Future + Send> ServiceFuture<F> {
    pub fn new(future: F, inner: Arc<LifelineInner>) -> Self {
        Self { future, inner }
    }
}

impl<F: Future> Future for ServiceFuture<F> {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self.inner.cancel.load(Ordering::Relaxed) {
            return Poll::Ready(());
        }

        // attempt to complete the future
        if let Poll::Ready(_) = self.as_mut().project().future.poll(cx) {
            return Poll::Ready(());
        }

        // Register to receive a wakeup if the future is aborted in the... future
        self.inner.waker.register(cx.waker());

        // Check to see if the future was aborted between the first check and
        // registration.
        // Checking with `Relaxed` is sufficient because `register` introduces an
        // `AcqRel` barrier.
        if self.inner.cancel.load(Ordering::Relaxed) {
            return Poll::Ready(());
        }

        Poll::Pending
    }
}

pub struct Lifeline {
    inner: Arc<LifelineInner>,
}

impl Lifeline {
    pub(crate) fn new(inner: Arc<LifelineInner>) -> Self {
        Self { inner }
    }
}

impl Drop for Lifeline {
    fn drop(&mut self) {
        self.inner.abort();
    }
}

#[derive(Debug)]
pub(crate) struct LifelineInner {
    waker: AtomicWaker,
    cancel: AtomicBool,
}

impl LifelineInner {
    pub fn new() -> Self {
        LifelineInner {
            waker: AtomicWaker::new(),
            cancel: AtomicBool::new(false),
        }
    }

    pub fn abort(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.waker.wake();
    }
}
