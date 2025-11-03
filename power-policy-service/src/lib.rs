#![no_std]
use core::ops::DerefMut;
use embassy_sync::mutex::Mutex;
use embedded_services::GlobalRawMutex;
use embedded_services::power::policy::device::{Device, DeviceTrait, State};
use embedded_services::power::policy::policy::EventReceiver;
use embedded_services::power::policy::{policy, *};
use embedded_services::sync::Lockable;
use embedded_services::{comms, error, info};

pub mod config;
pub mod consumer;
pub mod provider;

pub use config::Config;
pub mod charger;

#[derive(Copy, Clone, Default)]
struct InternalState {
    /// Current consumer state, if any
    current_consumer_state: Option<consumer::AvailableConsumer>,
    /// Current provider global state
    current_provider_state: provider::State,
    /// System unconstrained power
    unconstrained: UnconstrainedState,
}

/// Power policy state
pub struct PowerPolicy<D: Lockable, R: EventReceiver>
where
    D::Inner: DeviceTrait,
{
    /// Power policy context
    context: policy::ContextToken<D, R>,
    /// State
    state: Mutex<GlobalRawMutex, InternalState>,
    /// Comms endpoint
    tp: comms::Endpoint,
    /// Config
    config: config::Config,
}

impl<D: Lockable + 'static, R: EventReceiver + 'static> PowerPolicy<D, R>
where
    D::Inner: DeviceTrait,
{
    /// Create a new power policy
    pub fn create(config: config::Config) -> Option<Self> {
        Some(Self {
            context: policy::ContextToken::create()?,
            state: Mutex::new(InternalState::default()),
            tp: comms::Endpoint::uninit(comms::EndpointID::Internal(comms::Internal::Power)),
            config,
        })
    }

    async fn process_notify_attach(&self) -> Result<(), Error> {
        Ok(())
    }

    async fn process_notify_detach(&self) -> Result<(), Error> {
        self.update_current_consumer().await?;
        Ok(())
    }

    async fn process_notify_consumer_power_capability(
        &self,
        device: DeviceId,
        capability: Option<ConsumerPowerCapability>,
    ) -> Result<(), Error> {
        let device = self.context.get_device(device).await?;
        match device.state().await {
            State::Idle | State::ConnectedConsumer(_) | State::ConnectedProvider(_) => {
                device.state.lock().await.consumer_capability = capability;
                self.update_current_consumer().await
            }
            State::Detached => device.device.lock().await.disconnect().await,
        }
    }

    async fn process_request_provider_power_capabilities(
        &self,
        device: DeviceId,
        capability: Option<ProviderPowerCapability>,
    ) -> Result<(), Error> {
        let device = self.context.get_device(device).await?;
        match device.state().await {
            State::Idle | State::ConnectedConsumer(_) | State::ConnectedProvider(_) => {
                device.state.lock().await.requested_provider_capability = capability;
                self.connect_provider(device.id()).await
            }
            State::Detached => device.device.lock().await.disconnect().await,
        }
    }

    async fn process_notify_disconnect(&self) -> Result<(), Error> {
        if let Some(consumer) = self.state.lock().await.current_consumer_state.take() {
            info!("Device{}: Connected consumer disconnected", consumer.device_id.0);
            self.disconnect_chargers().await?;

            self.comms_notify(CommsMessage {
                data: CommsData::ConsumerDisconnected(consumer.device_id),
            })
            .await;
        }

        self.update_current_consumer().await?;
        Ok(())
    }

    /// Send a notification with the comms service
    async fn comms_notify(&self, message: CommsMessage) {
        self.context.broadcast_message(message).await;
        let _ = self
            .tp
            .send(comms::EndpointID::Internal(comms::Internal::Battery), &message)
            .await;
    }

    async fn wait_request(&self) -> policy::Request {
        self.context.wait_request().await
    }

    async fn process_request(&self, request: policy::Request) -> Result<(), Error> {
        let device = self.context.get_device(request.id).await?;

        match request.data {
            policy::RequestData::Attached => {
                info!("Received notify attached from device {}", device.id().0);
                self.process_notify_attach().await
            }
            policy::RequestData::Detached => {
                info!("Received notify detached from device {}", device.id().0);
                self.process_notify_detach().await
            }
            policy::RequestData::UpdatedConsumerCapability(capability) => {
                info!(
                    "Device{}: Received notify consumer capability: {:#?}",
                    device.id().0,
                    capability,
                );
                self.process_notify_consumer_power_capability(device.id(), capability)
                    .await
            }
            policy::RequestData::RequestedProviderCapability(capability) => {
                info!(
                    "Device{}: Received request provider capability: {:#?}",
                    device.id().0,
                    capability,
                );
                self.process_request_provider_power_capabilities(device.id(), capability)
                    .await
            }
            policy::RequestData::Disconnected => {
                info!("Received notify disconnect from device {}", device.id().0);
                self.process_notify_disconnect().await
            }
        }
    }

    /// Top-level event loop function
    pub async fn process(&self) -> Result<(), Error> {
        let request = self.wait_request().await;
        self.process_request(request).await
    }
}

impl<D: Lockable + 'static, R: EventReceiver + 'static> comms::MailboxDelegate for PowerPolicy<D, R> where
    D::Inner: DeviceTrait
{
}
