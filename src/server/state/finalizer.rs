use std::pin::Pin;
use std::task::{Context, Poll};

pub(crate) struct FinalizerToken {
    inner: tokio::task::JoinHandle<()>,
}

impl FinalizerToken {
    /// Creates a dummy completed finalizer token that resolves instantly.
    pub(crate) fn dummy() -> Self {
        Self {
            inner: tokio::spawn(async {}),
        }
    }

    pub(crate) fn from_handle(task_handle: tokio::task::JoinHandle<()>) -> Self {
        Self { inner: task_handle }
    }
}

impl Future for FinalizerToken {
    type Output = Result<(), tokio::task::JoinError>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.inner).poll(cx)
    }
}
