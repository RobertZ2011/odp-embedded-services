//! This module provides a layer that only has immutable access to message and reponse values.

use super::Layer;
use crate::transformers::{Component, Entity, RefGuard, RefMutGuard};
use core::{
    cell::RefCell,
    future::{self, Future},
};

///
pub trait Observe {
    /// Message type that we're waiting for
    type Message;
    /// Response type
    type Response;

    /// Default implementation that does nothing
    fn observe_message(&self, _message: &Self::Message) -> impl Future<Output = ()> {
        future::ready(())
    }

    /// Default implementation that does nothing
    fn observe_response(&self, _response: &Self::Response) -> impl Future<Output = ()> {
        future::ready(())
    }
}

/// Layer that wraps a single observer
pub struct ObserverLayer<L: Layer, O: Observe<Message = L::Message, Response = L::Response>> {
    inner: L,
    observe: RefCell<O>,
}

impl<L: Layer, O: Observe<Message = L::Message, Response = L::Response>> ObserverLayer<L, O> {
    /// Create a new observer layer
    pub fn new(inner: L, observe: O) -> Self {
        Self {
            inner,
            observe: RefCell::new(observe),
        }
    }

    /// Create a closure that can be used to wrap a layer in [`Self`]
    pub fn with_observer(observer: O) -> impl FnOnce(L) -> Self {
        |l| ObserverLayer::new(l, observer)
    }
}

impl<L: Layer, O: Observe<Message = L::Message, Response = L::Response>> Entity for ObserverLayer<L, O> {
    type Inner = L::Inner;

    fn get_entity(&self) -> impl RefGuard<Self::Inner> {
        self.inner.get_entity()
    }

    fn get_entity_mut(&self) -> impl RefMutGuard<Self::Inner> {
        self.inner.get_entity_mut()
    }
}

impl<L: Layer, O: Observe<Message = L::Message, Response = L::Response>> Component<L::Inner> for ObserverLayer<L, O> {
    type Message = L::Message;
    type Response = L::Response;

    #[inline]
    async fn wait_message(&self, entity: &L::Inner) -> Self::Message {
        let message = self.inner.wait_message(entity).await;
        self.observe.borrow_mut().observe_message(&message).await;
        message
    }

    #[inline]
    async fn process(&self, entity: &mut L::Inner, event: Self::Message) -> Self::Response {
        let response = self.inner.process(entity, event).await;
        self.observe.borrow_mut().observe_response(&response).await;
        response
    }

    #[inline]
    async fn send_response(&self, response: Self::Response) {
        self.inner.send_response(response).await;
    }
}

impl<L: Layer, O: Observe<Message = L::Message, Response = L::Response>> Layer for ObserverLayer<L, O> {}
