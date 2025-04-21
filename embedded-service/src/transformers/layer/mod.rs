//! Layer trait and associated functionality
use core::{future::Future, ops::DerefMut};

use super::{
    result::{Nested1, Nested2},
    Component, Entity,
};

pub mod component;
pub use component::*;
pub mod modify;
pub use modify::*;
pub mod observe;
pub use observe::*;
pub mod r#override;
pub use r#override::*;

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

/// Marker trait to allow for enforcing type constraints on layers
pub trait MessageTypedLayer1<A>: Layer<Message = Nested1<A>> {}
impl<A, L: Layer<Message = Nested1<A>>> MessageTypedLayer1<A> for L {}

/// Marker trait to allow for enforcing type constraints on layers
pub trait MessageTypedLayer2<A, B>: Layer<Message = Nested2<A, B>> {}
impl<A, B, L: Layer<Message = Nested2<A, B>>> MessageTypedLayer2<A, B> for L {}

/// Marker trait to allow for enforcing type constraints on layers
pub trait MessageTypedLayer3<A, B, C>: Layer<Message = Nested2<A, B>> {}
impl<A, B, C, L: Layer<Message = Nested2<A, B>>> MessageTypedLayer3<A, B, C> for L {}

/// Marker trait to allow for enforcing type constraints on layers
pub trait MessageTypedLayer4<A, B, C, D>: Layer<Message = Nested2<A, B>> {}
impl<A, B, C, D, L: Layer<Message = Nested2<A, B>>> MessageTypedLayer4<A, B, C, D> for L {}
