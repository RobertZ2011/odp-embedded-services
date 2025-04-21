//! Embedded Entity-Component System (ECS)
use core::{
    borrow::BorrowMut,
    cell::{Ref, RefCell, RefMut},
    future::{self, Future},
    ops::{Deref, DerefMut},
};

use embassy_futures::select::{select, Either};

/// Used to chain the results of each layer together
pub enum LayerResult<A, B> {
    /// Event for the current layer
    Head(A),
    /// Events for the rest of the layers
    Rest(B),
}

/// Type alias for a result from a single layer, the base entity will produce () as the result
pub type LayerResult1<A> = LayerResult<A, ()>;
/// Type alias for a result from two layers
pub type LayerResult2<A, B> = LayerResult<A, LayerResult1<B>>;

pub trait MessageTypedLayer1<A>: Layer<Message = LayerResult1<A>> {}
impl<A, L: Layer<Message = LayerResult1<A>>> MessageTypedLayer1<A> for L {}

pub trait MessageTypedLayer2<A, B>: Layer<Message = LayerResult2<A, B>> {}
impl<A, B, L: Layer<Message = LayerResult2<A, B>>> MessageTypedLayer2<A, B> for L {}

pub enum ResultView0<'a, A> {
    Result0(&'a A),
}
pub enum ResultViewMut0<'a, A> {
    Result0(&'a mut A),
}

impl<'a, A> TryFrom<&'a LayerResult1<A>> for ResultView0<'a, A> {
    type Error = ();

    fn try_from(value: &'a LayerResult1<A>) -> Result<Self, Self::Error> {
        match value {
            LayerResult::Head(a) => Ok(ResultView0::Result0(a)),
            LayerResult::Rest(_) => Err(()),
        }
    }
}

impl<'a, A> TryFrom<&'a mut LayerResult1<A>> for ResultViewMut0<'a, A> {
    type Error = ();

    fn try_from(value: &'a mut LayerResult1<A>) -> Result<Self, Self::Error> {
        match value {
            LayerResult::Head(a) => Ok(ResultViewMut0::Result0(a)),
            LayerResult::Rest(_) => Err(()),
        }
    }
}

pub struct Index0;
pub struct Index1;
pub struct Index2;

trait Index {}

impl Index for Index0 {}
impl Index for Index1 {}
impl Index for Index2 {}

pub trait Get<T, I: Index> {
    /// Get the inner type
    fn get(&self) -> Option<&T>;
}

impl<'a, A> Get<A, Index0> for ResultView0<'a, A> {
    fn get(&self) -> Option<&A> {
        match self {
            ResultView0::Result0(a) => Some(a),
        }
    }
}

impl<'a, A, B> Get<A, Index0> for ResultView1<'a, A, B> {
    fn get(&self) -> Option<&A> {
        match self {
            ResultView1::Result0(a) => Some(a),
            ResultView1::Result1(_) => None,
        }
    }
}
impl<'a, A, B> Get<B, Index1> for ResultView1<'a, A, B> {
    fn get(&self) -> Option<&B> {
        match self {
            ResultView1::Result0(_) => None,
            ResultView1::Result1(b) => Some(b),
        }
    }
}

pub enum ResultView1<'a, A, B> {
    Result0(&'a A),
    Result1(&'a B),
}

impl<'a, A, B> TryFrom<&'a LayerResult2<A, B>> for ResultView1<'a, A, B> {
    type Error = ();

    fn try_from(value: &'a LayerResult2<A, B>) -> Result<Self, Self::Error> {
        match value {
            LayerResult::Head(a) => Ok(ResultView1::Result0(a)),
            LayerResult::Rest(b) => match b.try_into() {
                Ok(ResultView0::Result0(b)) => Ok(ResultView1::Result1(b)),
                Err(_) => Err(()),
            },
        }
    }
}
pub enum ResultViewMut1<'a, A, B> {
    Result0(&'a mut A),
    Result1(&'a mut B),
}

pub enum ResultView2<'a, A, B> {
    Result0(&'a A),
    Result1(&'a B),
}
pub enum ResultViewMut2<'a, A, B> {
    Result0(&'a mut A),
    Result1(&'a mut B),
}

pub enum ResultView3<'a, A, B, C> {
    Result0(&'a A),
    Result1(&'a B),
    Result2(&'a C),
}
pub enum ResultViewMut3<'a, A, B, C> {
    Result0(&'a mut A),
    Result1(&'a mut B),
    Result2(&'a mut C),
}

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
    type Message = LayerResult<C::Message, L::Message>;
    type Response = LayerResult<C::Response, L::Response>;

    #[inline]
    async fn wait_message(&self, entity: &L::Inner) -> Self::Message {
        let mut borrow = self.component.borrow_mut();
        let component = borrow.deref_mut();

        match select(component.wait_message(entity), self.inner.wait_message(entity)).await {
            Either::First(event) => LayerResult::Head(event),
            Either::Second(event) => LayerResult::Rest(event),
        }
    }

    #[inline]
    async fn process(&self, entity: &mut L::Inner, event: Self::Message) -> Self::Response {
        let mut borrow = self.component.borrow_mut();
        let component = borrow.deref_mut();

        match event {
            LayerResult::Head(event) => LayerResult::Head(component.process(entity, event).await),
            LayerResult::Rest(event) => LayerResult::Rest(self.inner.process(entity, event).await),
        }
    }

    #[inline]
    async fn send_response(&self, response: Self::Response) {
        let mut borrow = self.component.borrow_mut();
        let component = borrow.deref_mut();

        match response {
            LayerResult::Head(response) => component.send_response(response).await,
            LayerResult::Rest(response) => self.inner.send_response(response).await,
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
