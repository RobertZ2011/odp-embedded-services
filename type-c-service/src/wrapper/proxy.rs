use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::channel::{Channel, DynamicReceiver, DynamicSender};
use embedded_services::named::Named;
use embedded_usb_pd::PdError;
use power_policy_interface::psu::{CommandData as PolicyCommandData, InternalResponseData as PolicyResponseData, Psu};
use type_c_interface::port::{PortCommandData, PortResponseData};

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PortProxyCommandData {
    Power(PolicyCommandData),
    Port(PortCommandData),
}

pub enum PortProxyResponseData {
    Power(PolicyResponseData),
    Port(Result<PortResponseData, PdError>),
}

impl From<PortProxyResponseData> for Result<(), power_policy_interface::psu::Error> {
    fn from(value: PortProxyResponseData) -> Self {
        match value {
            PortProxyResponseData::Power(response) => response?.complete_or_err(),
            PortProxyResponseData::Port(_) => Err(power_policy_interface::psu::Error::InvalidResponse),
        }
    }
}

pub struct PowerProxyChannel<M: RawMutex> {
    command_channel: Channel<M, PortProxyCommandData, 1>,
    response_channel: Channel<M, PortProxyResponseData, 1>,
}

impl<M: RawMutex> PowerProxyChannel<M> {
    pub fn new() -> Self {
        Self {
            command_channel: Channel::new(),
            response_channel: Channel::new(),
        }
    }

    pub fn get_device_components(
        &self,
    ) -> (
        DynamicSender<'_, PortProxyCommandData>,
        DynamicReceiver<'_, PortProxyResponseData>,
    ) {
        (self.command_channel.dyn_sender(), self.response_channel.dyn_receiver())
    }

    pub fn get_receiver(&self) -> PowerProxyReceiver<'_> {
        PowerProxyReceiver {
            receiver: self.command_channel.dyn_receiver(),
            sender: self.response_channel.dyn_sender(),
        }
    }
}

pub struct PowerProxyReceiver<'a> {
    sender: DynamicSender<'a, PortProxyResponseData>,
    receiver: DynamicReceiver<'a, PortProxyCommandData>,
}

impl<'a> PowerProxyReceiver<'a> {
    pub fn new(
        receiver: DynamicReceiver<'a, PortProxyCommandData>,
        sender: DynamicSender<'a, PortProxyResponseData>,
    ) -> Self {
        Self { receiver, sender }
    }

    pub async fn receive(&mut self) -> PortProxyCommandData {
        self.receiver.receive().await
    }

    pub async fn send(&mut self, response: PortProxyResponseData) {
        self.sender.send(response).await;
    }
}

pub struct PowerProxyDevice<'a> {
    sender: DynamicSender<'a, PortProxyCommandData>,
    receiver: DynamicReceiver<'a, PortProxyResponseData>,
    /// Per-port PSU state
    pub(crate) psu_state: power_policy_interface::psu::State,
    name: &'static str,
}

impl<'a> PowerProxyDevice<'a> {
    pub fn new(
        name: &'static str,
        sender: DynamicSender<'a, PortProxyCommandData>,
        receiver: DynamicReceiver<'a, PortProxyResponseData>,
    ) -> Self {
        Self {
            name,
            sender,
            receiver,
            psu_state: power_policy_interface::psu::State::default(),
        }
    }

    async fn execute(&mut self, command: PortProxyCommandData) -> PortProxyResponseData {
        self.sender.send(command).await;
        self.receiver.receive().await
    }
}

impl<'a> Psu for PowerProxyDevice<'a> {
    async fn disconnect(&mut self) -> Result<(), power_policy_interface::psu::Error> {
        self.execute(PortProxyCommandData::Power(PolicyCommandData::Disconnect))
            .await
            .into()
    }

    async fn connect_provider(
        &mut self,
        capability: power_policy_interface::capability::ProviderPowerCapability,
    ) -> Result<(), power_policy_interface::psu::Error> {
        self.execute(PortProxyCommandData::Power(PolicyCommandData::ConnectAsProvider(
            capability,
        )))
        .await
        .into()
    }

    async fn connect_consumer(
        &mut self,
        capability: power_policy_interface::capability::ConsumerPowerCapability,
    ) -> Result<(), power_policy_interface::psu::Error> {
        self.execute(PortProxyCommandData::Power(PolicyCommandData::ConnectAsConsumer(
            capability,
        )))
        .await
        .into()
    }

    fn state(&self) -> &power_policy_interface::psu::State {
        &self.psu_state
    }

    fn state_mut(&mut self) -> &mut power_policy_interface::psu::State {
        &mut self.psu_state
    }
}

impl<'a> Named for PowerProxyDevice<'a> {
    fn name(&self) -> &'static str {
        self.name
    }
}

impl<M: RawMutex> Default for PowerProxyChannel<M> {
    fn default() -> Self {
        Self::new()
    }
}
