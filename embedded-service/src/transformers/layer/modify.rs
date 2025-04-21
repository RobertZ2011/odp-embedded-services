//! Module for a layer that can modify messages and responses

use core::cell::RefCell;

use crate::transformers::{Component, Entity, RefGuard, RefMutGuard};

use super::Layer;

///
pub trait Modify {
    /// Message type that we're waiting for
    type Message;
    /// Response type
    type Response;

    /// Default implementation that does nothing
    fn modify_message(&self, _message: &mut Self::Message) {}

    /// Default implementation that does nothing
    fn modify_response(&self, _response: &mut Self::Response) {}
}

/// Layer that wraps a single modifier
pub struct ModifierLayer<L: Layer, M: Modify<Message = L::Message, Response = L::Response>> {
    inner: L,
    observe: RefCell<M>,
}

impl<L: Layer, M: Modify<Message = L::Message, Response = L::Response>> ModifierLayer<L, M> {
    /// Create a new modifier layer
    pub fn new(inner: L, observe: M) -> Self {
        Self {
            inner,
            observe: RefCell::new(observe),
        }
    }

    /// Create a closure that can be used to wrap a layer in [`Self`]
    pub fn with_modifier(modifier: M) -> impl FnOnce(L) -> Self {
        |l| ModifierLayer::new(l, modifier)
    }
}

impl<L: Layer, M: Modify<Message = L::Message, Response = L::Response>> Entity for ModifierLayer<L, M> {
    type Inner = L::Inner;

    fn get_entity(&self) -> impl RefGuard<Self::Inner> {
        self.inner.get_entity()
    }

    fn get_entity_mut(&self) -> impl RefMutGuard<Self::Inner> {
        self.inner.get_entity_mut()
    }
}

impl<L: Layer, M: Modify<Message = L::Message, Response = L::Response>> Component<L::Inner> for ModifierLayer<L, M> {
    type Message = L::Message;
    type Response = L::Response;

    #[inline]
    async fn wait_message(&self, entity: &L::Inner) -> Self::Message {
        let mut message = self.inner.wait_message(entity).await;
        self.observe.borrow_mut().modify_message(&mut message);
        message
    }

    #[inline]
    async fn process(&self, entity: &mut L::Inner, event: Self::Message) -> Self::Response {
        let mut response = self.inner.process(entity, event).await;
        self.observe.borrow_mut().modify_response(&mut response);
        response
    }

    #[inline]
    async fn send_response(&self, response: Self::Response) {
        self.inner.send_response(response).await;
    }
}

impl<L: Layer, M: Modify<Message = L::Message, Response = L::Response>> Layer for ModifierLayer<L, M> {}
