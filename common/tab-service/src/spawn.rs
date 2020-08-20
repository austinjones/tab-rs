use futures::{task::AtomicWaker, Future};
use std::fmt::Debug;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::Poll,
};

use crate::type_name::type_name;
use log::debug;
use pin_project::pin_project;

// TODO: allow Result from the spawn

/// Executes the task, until the future completes, or the lifeline is dropped
pub(crate) fn spawn_task<Service, F, O>(task_name: &str, fut: F) -> Lifeline
where
    F: Future<Output = O> + Send + 'static,
    O: Debug + Send + 'static,
{
    let inner = Arc::new(LifelineInner::new());

    let service_name = type_name::<Service>().to_string() + "/" + task_name;

    let service = ServiceFuture::new(service_name, fut, inner.clone());
    spawn_task_inner(service);

    Lifeline::new(inner)
}

// pub fn spawn_from_stream<T, S, F, Fut>(mut stream: S, mut f: F) -> Lifeline
// where
//     S: Stream<Item = T> + Send + Unpin + 'static,
//     F: FnMut(T) -> Fut + Send + Sync + 'static,
//     T: Send,
//     Fut: Future + Send,
// {
//     let future = async move {
//         while let Some(msg) = stream.next().await {
//             f(msg).await;
//         }
//     };

//     spawn(future)
// }

// pub fn spawn_from<State, Output, Fut, F>(state: State, function: F) -> Lifeline
// where
//     Output: Send + 'static,
//     Fut: Future<Output = Output> + Send + 'static,
//     F: FnOnce(State) -> Fut,
// {
//     let future = function(state);
//     spawn(future)
// }

// #[cfg(feature = "tokio-executor")]
fn spawn_task_inner<F, O>(task: F)
where
    F: Future<Output = O> + Send + 'static,
    O: Send + 'static,
{
    tokio::spawn(task);
}

#[pin_project]
struct ServiceFuture<F: Future> {
    #[pin]
    future: F,
    name: String,
    inner: Arc<LifelineInner>,
}

impl<F: Future + Send> ServiceFuture<F> {
    pub fn new(name: String, future: F, inner: Arc<LifelineInner>) -> Self {
        debug!("START {}", &name);

        Self {
            name,
            future,
            inner,
        }
    }
}

impl<F: Future> Future for ServiceFuture<F>
where
    F::Output: Debug,
{
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self.inner.cancel.load(Ordering::Relaxed) {
            debug!("CANCEL {}", self.name);
            return Poll::Ready(());
        }

        // attempt to complete the future
        if let Poll::Ready(result) = self.as_mut().project().future.poll(cx) {
            debug!("FINISH {} {:?}", self.name, result);
            return Poll::Ready(());
        }

        // Register to receive a wakeup if the future is aborted in the... future
        self.inner.waker.register(cx.waker());

        // Check to see if the future was aborted between the first check and
        // registration.
        // Checking with `Relaxed` is sufficient because `register` introduces an
        // `AcqRel` barrier.
        if self.inner.cancel.load(Ordering::Relaxed) {
            debug!("CANCEL {}", self.name);
            return Poll::Ready(());
        }

        Poll::Pending
    }
}

#[must_use = "if unused the service will immediately be cancelled"]
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
