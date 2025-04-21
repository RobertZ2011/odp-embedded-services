//! Embedded Entity-Component System (ECS)
use core::{
    cell::{Ref, RefCell, RefMut},
    future::{self, Future},
    ops::{Deref, DerefMut},
};

pub mod layer;
pub mod result;

use layer::Layer;

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
