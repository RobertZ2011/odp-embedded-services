//! Simple layer that wraps a single component
use core::{cell::RefCell, ops::DerefMut};

use embassy_futures::select::{select, Either};

use crate::transformers::{result::Nested, Component, Entity, RefGuard, RefMutGuard};

use super::Layer;

/// Layer that wraps a single component
pub struct ComponentLayer<L: Layer, C: Component<L::Inner>> {
    inner: L,
    component: RefCell<C>,
}

impl<L: Layer, C: Component<L::Inner>> Entity for ComponentLayer<L, C> {
    type Inner = L::Inner;

    fn get_entity(&self) -> impl RefGuard<Self::Inner> {
        self.inner.get_entity()
    }

    fn get_entity_mut(&self) -> impl RefMutGuard<Self::Inner> {
        self.inner.get_entity_mut()
    }
}

impl<L: Layer, C: Component<L::Inner>> Component<L::Inner> for ComponentLayer<L, C> {
    type Message = Nested<C::Message, L::Message>;
    type Response = Nested<C::Response, L::Response>;

    #[inline]
    async fn wait_message(&self, entity: &L::Inner) -> Self::Message {
        let mut borrow = self.component.borrow_mut();
        let component = borrow.deref_mut();

        match select(component.wait_message(entity), self.inner.wait_message(entity)).await {
            Either::First(event) => Nested::Some(event),
            Either::Second(event) => Nested::Other(event),
        }
    }

    #[inline]
    async fn process(&self, entity: &mut L::Inner, event: Self::Message) -> Self::Response {
        let mut borrow = self.component.borrow_mut();
        let component = borrow.deref_mut();

        match event {
            Nested::Some(event) => Nested::Some(component.process(entity, event).await),
            Nested::Other(event) => Nested::Other(self.inner.process(entity, event).await),
        }
    }

    #[inline]
    async fn send_response(&self, response: Self::Response) {
        let mut borrow = self.component.borrow_mut();
        let component = borrow.deref_mut();

        match response {
            Nested::Some(response) => component.send_response(response).await,
            Nested::Other(response) => self.inner.send_response(response).await,
        }
    }
}

impl<L: Layer, C: Component<L::Inner>> Layer for ComponentLayer<L, C> {}

impl<L: Layer, C: Component<L::Inner>> ComponentLayer<L, C> {
    /// Create a new layer wrapper
    pub fn new(layer: L, component: C) -> Self {
        Self {
            inner: layer,
            component: RefCell::new(component),
        }
    }

    /// Create a closure that can be used to wrap a layer in [`Self`]
    pub fn with_component(component: C) -> impl FnOnce(L) -> Self {
        |l| ComponentLayer::new(l, component)
    }
}
