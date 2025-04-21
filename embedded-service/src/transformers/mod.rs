//! Embedded Entity-Component System (ECS)
use core::{
    cell::{Ref, RefCell, RefMut},
    future::{self, Future},
    ops::{Deref, DerefMut},
};

pub mod result;

use embassy_futures::select::{select, Either};
use result::{Nested, Nested1, Nested2};

pub trait MessageTypedLayer1<A>: Layer<Message = Nested1<A>> {}
impl<A, L: Layer<Message = Nested1<A>>> MessageTypedLayer1<A> for L {}

pub trait MessageTypedLayer2<A, B>: Layer<Message = Nested2<A, B>> {}
impl<A, B, L: Layer<Message = Nested2<A, B>>> MessageTypedLayer2<A, B> for L {}

/// Trait to allow for borrowing a reference to the inner type
pub trait RefGuard<T>: Deref<Target = T> {}

/// Trait to allow for borrowing a mutable reference to the inner type
pub trait RefMutGuard<T>: DerefMut<Target = T> {}

/// Core component type
pub trait Component<E> {
    /// Message type that we're waiting for
    type Message;
    /// Response type
    type Response;

    /// Wait for a message
    fn wait_message(&self, entity: &E) -> impl Future<Output = Self::Message>;
    /// Process the event
    fn process(&self, entity: &mut E, event: Self::Message) -> impl Future<Output = Self::Response>;
    /// Send a response to the message
    fn send_response(&self, response: Self::Response) -> impl Future<Output = ()>;
}

/// Entity trait
pub trait Entity {
    /// Underlying type of the entity
    type Inner;

    /// Get a reference to the inner entity
    fn get_entity(&self) -> impl RefGuard<Self::Inner>;
    /// Get a mutable reference to the inner entity
    fn get_entity_mut(&self) -> impl RefMutGuard<Self::Inner>;
}

/// Layer trait
pub trait Layer: Entity + Component<Self::Inner> + Sized {
    /// Process all events for the layer and layers below it
    fn process_all(&mut self) -> impl Future<Output = ()> {
        async {
            let mut borrow = self.get_entity_mut();
            let entity = borrow.deref_mut();
            let event = self.wait_message(entity).await;
            let response = self.process(entity, event).await;
            self.send_response(response).await;
        }
    }

    /// Wrap the layer in a new layer
    fn add_layer<O: Layer>(self, f: impl FnOnce(Self) -> O) -> O {
        f(self)
    }
}

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

    pub fn with_component(component: C) -> impl FnOnce(L) -> Self {
        |l| ComponentLayer::new(l, component)
    }
}

/// Entity that stores its value in a RefCell
pub struct EntityRefCell<E> {
    inner: RefCell<E>,
}

impl<T> RefGuard<T> for Ref<'_, T> {}
impl<T> RefMutGuard<T> for RefMut<'_, T> {}

impl<E> EntityRefCell<E> {
    /// Create a new entity reference
    pub fn new(entity: E) -> Self {
        Self {
            inner: RefCell::new(entity),
        }
    }

    /// Wrap the entity in a new component layer
    pub fn add_component<C: Component<E>>(self, component: C) -> ComponentLayer<Self, C> {
        ComponentLayer::new(self, component)
    }
}

impl<T> Entity for EntityRefCell<T> {
    type Inner = T;

    #[inline]
    fn get_entity(&self) -> impl RefGuard<Self::Inner> {
        self.inner.borrow()
    }

    #[inline]
    fn get_entity_mut(&self) -> impl RefMutGuard<Self::Inner> {
        self.inner.borrow_mut()
    }
}

impl<E> Component<E> for EntityRefCell<E> {
    type Message = ();
    type Response = ();

    #[inline]
    async fn wait_message(&self, _: &E) -> Self::Message {
        future::pending().await
    }

    #[inline]
    async fn process(&self, _: &mut E, _: Self::Message) {
        ()
    }

    #[inline]
    async fn send_response(&self, _: Self::Response) {
        ()
    }
}

impl<T> Layer for EntityRefCell<T> {
    async fn process_all(&mut self) {
        ()
    }
}
