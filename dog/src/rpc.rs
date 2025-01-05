use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

use futures::{stream::Peekable, Stream, StreamExt};

use crate::types::RpcOut;

/// `RpcOut` sender that is priority aware.
#[derive(Debug)]
pub(crate) struct Sender {
    /// Capacity of the priority channel for `Publish` transactions.
    priority_cap: usize,
    len: Arc<AtomicUsize>,
    pub(crate) priority_sender: async_channel::Sender<RpcOut>,
    pub(crate) non_priority_sender: async_channel::Sender<RpcOut>,
    priority_receiver: async_channel::Receiver<RpcOut>,
    non_priority_receiver: async_channel::Receiver<RpcOut>,
}

impl Sender {
    /// Create a Rpc Sender.
    pub(crate) fn new(cap: usize) -> Sender {
        // We intentionally do not bound the channel, as we still need to send control messages
        // such as `HaveTx` and `ResetRoute`.
        // That's also why we define `cap` and divide it by two.
        // to ensure there is capacity for both priority and non-priority messages.
        let len = Arc::new(AtomicUsize::new(0));
        let (priority_sender, priority_receiver) = async_channel::unbounded();
        let (non_priority_sender, non_priority_receiver) = async_channel::bounded(cap / 2);
        Sender {
            priority_cap: cap / 2,
            len,
            priority_sender,
            non_priority_sender,
            priority_receiver,
            non_priority_receiver,
        }
    }

    /// Create a new Receiver to the sender.
    pub(crate) fn new_receiver(&self) -> Receiver {
        Receiver {
            priority_queue_len: self.len.clone(),
            priority_receiver: Box::pin(self.priority_receiver.clone().peekable()),
            non_priority_receiver: Box::pin(self.non_priority_receiver.clone().peekable()),
        }
    }

    pub(crate) fn send_transaction(&self, rpc: RpcOut) -> Result<(), RpcOut> {
        if let RpcOut::Publish { .. } = rpc {
            let len = self.len.load(Ordering::Relaxed);
            if len >= self.priority_cap {
                return Err(rpc);
            }
            self.len.store(len + 1, Ordering::Relaxed);
        }
        let sender = match rpc {
            RpcOut::Publish { .. } | RpcOut::HaveTx(_) | RpcOut::ResetRoute(_) => {
                &self.priority_sender
            }
            RpcOut::Forward { .. } => &self.non_priority_sender,
        };
        sender.try_send(rpc).map_err(|err| err.into_inner())
    }

    // pub(crate) fn priority_queue_len(&self) -> usize {
    //     self.len.load(Ordering::Relaxed)
    // }

    // pub(crate) fn non_priority_queue_len(&self) -> usize {
    //     self.non_priority_receiver.len()
    // }
}

/// `RpcOut` receiver that is priority aware.
#[derive(Debug)]
pub struct Receiver {
    pub(crate) priority_queue_len: Arc<AtomicUsize>,
    pub(crate) priority_receiver: Pin<Box<Peekable<async_channel::Receiver<RpcOut>>>>,
    pub(crate) non_priority_receiver: Pin<Box<Peekable<async_channel::Receiver<RpcOut>>>>,
}

impl Receiver {
    // Peek the next transaction in the queues and return it if its timeout has elapsed.
    // Returns `None` if there aren't any more transactions on the stream or none is stale.
    pub(crate) fn poll_stale(&mut self, cx: &mut Context<'_>) -> Poll<Option<RpcOut>> {
        let priority = match self.priority_receiver.as_mut().poll_peek_mut(cx) {
            Poll::Ready(Some(RpcOut::Publish {
                tx: _,
                ref mut timeout,
            })) => {
                if Pin::new(timeout).poll(cx).is_ready() {
                    let dropped = futures::ready!(self.priority_receiver.poll_next_unpin(cx))
                        .expect("There should be a transaction");
                    return Poll::Ready(Some(dropped));
                }
                Poll::Ready(None)
            }
            poll => poll,
        };

        let non_priority = match self.non_priority_receiver.as_mut().poll_peek_mut(cx) {
            Poll::Ready(Some(RpcOut::Forward {
                tx: _,
                ref mut timeout,
            })) => {
                if Pin::new(timeout).poll(cx).is_ready() {
                    let dropped = futures::ready!(self.non_priority_receiver.poll_next_unpin(cx))
                        .expect("There should be a transaction");
                    return Poll::Ready(Some(dropped));
                }
                Poll::Ready(None)
            }
            poll => poll,
        };

        match (priority, non_priority) {
            (Poll::Ready(None), Poll::Ready(None)) => Poll::Ready(None),
            _ => Poll::Pending,
        }
    }

    /// Poll queues and return true if both are empty.
    pub(crate) fn poll_is_empty(&mut self, cx: &mut Context<'_>) -> bool {
        matches!(
            (
                self.priority_receiver.as_mut().poll_peek(cx),
                self.non_priority_receiver.as_mut().poll_peek(cx),
            ),
            (Poll::Ready(None), Poll::Ready(None))
        )
    }
}

impl Stream for Receiver {
    type Item = RpcOut;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if let Poll::Ready(rpc) = Pin::new(&mut self.priority_receiver).poll_next(cx) {
            if let Some(RpcOut::Publish { .. }) = rpc {
                self.priority_queue_len.fetch_sub(1, Ordering::Relaxed);
            }
            return Poll::Ready(rpc);
        }
        Pin::new(&mut self.non_priority_receiver).poll_next(cx)
    }
}
