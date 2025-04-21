//! Provides a layer that can override the default behavior provided by its inner layer.

use core::{
    cell::RefCell,
    future::{self, Future},
};

use crate::transformers::{Component, Entity, RefGuard, RefMutGuard};

use super::Layer;

///
pub trait Override {
    /// Message type
    type Message;
    /// Response type
    type Response;

    /// Default implementation that does nothing
    fn r#override(&self, _message: &Self::Message) -> impl Future<Output = Option<Self::Response>> {
        future::ready(None)
    }
}

/// Layer that wraps a single overrider
pub struct OverrideLayer<L: Layer, O: Override<Message = L::Message, Response = L::Response>> {
    inner: L,
    r#override: RefCell<O>,
}

impl<L: Layer, O: Override<Message = L::Message, Response = L::Response>> OverrideLayer<L, O> {
    /// Create a new observer layer
    pub fn new(inner: L, r#override: O) -> Self {
        Self {
            inner,
            r#override: RefCell::new(r#override),
        }
    }

    /// Create a closure that can be used to wrap a layer in [`Self`]
    pub fn with_override(r#overide: O) -> impl FnOnce(L) -> Self {
        |l| OverrideLayer::new(l, r#overide)
    }
}

impl<L: Layer, O: Override<Message = L::Message, Response = L::Response>> Entity for OverrideLayer<L, O> {
    type Inner = L::Inner;

    fn get_entity(&self) -> impl RefGuard<Self::Inner> {
        self.inner.get_entity()
    }

    fn get_entity_mut(&self) -> impl RefMutGuard<Self::Inner> {
        self.inner.get_entity_mut()
    }
}

impl<L: Layer, O: Override<Message = L::Message, Response = L::Response>> Component<L::Inner> for OverrideLayer<L, O> {
    type Message = L::Message;
    type Response = L::Response;

    #[inline]
    async fn wait_message(&self, entity: &L::Inner) -> Self::Message {
        self.inner.wait_message(entity).await
    }

    #[inline]
    async fn process(&self, entity: &mut L::Inner, event: Self::Message) -> Self::Response {
        if let Some(response) = self.r#override.borrow_mut().r#override(&event).await {
            response
        } else {
            // No override needed, return the original response
            self.inner.process(entity, event).await
        }
    }

    #[inline]
    async fn send_response(&self, response: Self::Response) {
        self.inner.send_response(response).await;
    }
}

impl<L: Layer, O: Override<Message = L::Message, Response = L::Response>> Layer for OverrideLayer<L, O> {}
