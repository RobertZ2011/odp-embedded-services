use embassy_executor::Executor;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::once_lock::OnceLock;
use embedded_services::transformers::{
    layer::{ComponentLayer, Layer, MessageTypedLayer2},
    result::{Get, Nested2},
    Component, Entity, EntityRefCell, RefGuard, RefMutGuard,
};
use log::info;
use static_cell::StaticCell;

pub struct Channel<M, R> {
    pub rx: embassy_sync::channel::Channel<NoopRawMutex, M, 1>,
    pub tx: embassy_sync::channel::Channel<NoopRawMutex, R, 1>,
}

static OPTIONAL_CHANNEL: OnceLock<Channel<optional::Message, optional::Response>> = OnceLock::new();
static CORE_CHANNEL: OnceLock<Channel<core::Message, core::Response>> = OnceLock::new();

impl<M, R> Channel<M, R> {
    pub fn new() -> Self {
        Self {
            tx: embassy_sync::channel::Channel::new(),
            rx: embassy_sync::channel::Channel::new(),
        }
    }

    pub async fn send_response(&self, message: R) {
        self.tx.send(message).await;
    }

    pub async fn receive_message(&self) -> M {
        self.rx.receive().await
    }

    pub async fn send_message(&self, message: M) {
        self.rx.send(message).await;
    }

    pub async fn receive_response(&self) -> R {
        self.tx.receive().await
    }

    pub async fn call(&self, message: M) -> R {
        self.send_message(message).await;
        self.receive_response().await
    }
}

mod core {
    use super::*;
    use embedded_services::transformers::Component;

    #[derive(Debug, Clone, Copy)]
    pub struct Message(pub i32);
    #[derive(Debug, Clone, Copy)]
    pub struct Response(pub i32);

    pub trait Trait {
        fn function(&self, value: i32) -> i32;
    }

    pub struct MessageBridge<'a> {
        pub channel: &'a Channel<Message, Response>,
    }

    impl MessageBridge<'_> {
        pub fn new() -> Self {
            Self {
                channel: CORE_CHANNEL.get_or_init(Channel::new),
            }
        }
    }

    impl<E> Component<E> for MessageBridge<'_>
    where
        E: Trait,
    {
        type Message = Message;
        type Response = Response;

        async fn wait_message(&self, _: &E) -> Self::Message {
            info!("Waiting for message...");
            self.channel.receive_message().await
        }

        async fn process(&self, entity: &mut E, event: Self::Message) -> Self::Response {
            info!("Processing message: {:?}", event.0);
            Response(entity.function(event.0))
        }

        async fn send_response(&self, response: Self::Response) -> () {
            self.channel.send_response(response).await;
        }
    }
}

mod optional {
    use super::*;
    use embedded_services::transformers::Component;

    #[derive(Debug, Clone, Copy)]
    pub struct Message(pub f32);
    #[derive(Debug, Clone, Copy)]
    pub struct Response(pub f32);

    pub trait Trait {
        fn function(&self, value: f32) -> f32;
    }

    pub struct MessageBridge<'a> {
        pub channel: &'a Channel<Message, Response>,
    }

    impl MessageBridge<'_> {
        pub fn new() -> Self {
            Self {
                channel: OPTIONAL_CHANNEL.get_or_init(Channel::new),
            }
        }
    }

    impl<T> Component<T> for MessageBridge<'_>
    where
        T: Trait,
    {
        type Message = Message;
        type Response = Response;

        async fn wait_message(&self, _: &T) -> Self::Message {
            info!("Waiting for optional message...");
            self.channel.receive_message().await
        }

        async fn process(&self, entity: &mut T, event: Self::Message) -> Self::Response {
            info!("Processing optional message: {:?}", event.0);
            Response(entity.function(event.0))
        }

        async fn send_response(&self, response: Self::Response) -> () {
            self.channel.send_response(response).await;
        }
    }
}

struct Device;

impl core::Trait for Device {
    fn function(&self, value: i32) -> i32 {
        value + 5
    }
}

impl optional::Trait for Device {
    fn function(&self, value: f32) -> f32 {
        1.5 * value
    }
}

struct TypedObserverLayer<L: MessageTypedLayer2<optional::Message, core::Message>> {
    inner: L,
}

impl<L: MessageTypedLayer2<optional::Message, core::Message>> TypedObserverLayer<L> {
    pub fn new(inner: L) -> Self {
        Self { inner }
    }
}

impl<L: MessageTypedLayer2<optional::Message, core::Message>> Entity for TypedObserverLayer<L> {
    type Inner = L::Inner;

    #[inline]
    fn get_entity(&self) -> impl RefGuard<Self::Inner> {
        self.inner.get_entity()
    }

    #[inline]
    fn get_entity_mut(&self) -> impl RefMutGuard<Self::Inner> {
        self.inner.get_entity_mut()
    }
}

impl<L: MessageTypedLayer2<optional::Message, core::Message>> Component<L::Inner> for TypedObserverLayer<L> {
    type Message = Nested2<optional::Message, core::Message>;
    type Response = L::Response;

    #[inline]
    async fn wait_message(&self, entity: &L::Inner) -> Self::Message {
        let l = self.inner.wait_message(entity).await;
        if let Some(m) = Get::<optional::Message, _>::get(&l) {
            info!("Got message: {:#?}", m);
        }

        l
    }

    #[inline]
    async fn process(&self, entity: &mut L::Inner, event: Self::Message) -> Self::Response {
        let response = self.inner.process(entity, event).await;
        info!("Processed message");
        response
    }

    #[inline]
    async fn send_response(&self, response: Self::Response) {
        self.inner.send_response(response).await;
        info!("Sent response");
    }
}

impl<L: MessageTypedLayer2<optional::Message, core::Message>> Layer for TypedObserverLayer<L> {}

#[embassy_executor::task]
async fn device_task() {
    let mut device = EntityRefCell::new(Device)
        .add_layer(ComponentLayer::with_component(core::MessageBridge::new()))
        .add_layer(ComponentLayer::with_component(optional::MessageBridge::new()))
        .add_layer(TypedObserverLayer::new);

    loop {
        device.process_all().await;
    }
}

#[embassy_executor::task]
async fn main_task() {
    let optional_channel = OPTIONAL_CHANNEL.get_or_init(Channel::new);
    let core_channel = CORE_CHANNEL.get_or_init(Channel::new);

    let message = core::Message(10);
    info!("Sending core message: {:?}", message);
    let response = core_channel.call(message).await;
    info!("Response: {:?}", response.0);

    let message = optional::Message(5.0);
    info!("Sending optional message: {:?}", message);
    let response = optional_channel.call(message).await;
    info!("Optional response: {:?}", response.0);
}

fn main() {
    env_logger::builder().filter_level(log::LevelFilter::Info).init();

    static EXECUTOR: StaticCell<Executor> = StaticCell::new();
    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.must_spawn(device_task());
        spawner.must_spawn(main_task());
    });
}
