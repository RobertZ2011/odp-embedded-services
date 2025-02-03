//! Power policy related data structures and messages

use embassy_sync::{
    blocking_mutex::raw::NoopRawMutex,
    channel::Channel,
    mutex::{Mutex, MutexGuard},
    once_lock::OnceLock,
};

use crate::{error, intrusive_list};

/// Error type
pub enum Error {
    /// The requested device does not exist
    InvalidDevice,
    /// The source request was denied, contains maximum available power
    CannotSource(Option<PowerCapability>),
    /// The sink request was denied, contains maximum available power
    CannotSink(Option<PowerCapability>),
    /// The device is not in the correct state
    InvalidState,
    /// Invalid response
    InvalidResponse,
    /// Bus error
    Bus,
    /// Generic failure
    Failed,
}

/// Device ID new type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DeviceId(pub u8);

/// Current state of the attached power device
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DeviceState {
    /// Device is attached, but is not currently sourcing or sinking power
    Attached,
    /// Device is attached and is currently sourcing power
    Source,
    /// Device is attached and is currently sinking power
    Sink,
    /// No device attached
    Detached,
}

/// Amount of power that a device can source or sink
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PowerCapability {
    /// Available voltage in mV
    pub voltage_mv: u16,
    /// Max available current in mA
    pub current_ma: u16,
}

impl PowerCapability {
    /// Calculate maximum power
    pub fn max_power_mw(&self) -> u32 {
        self.voltage_mv as u32 * self.current_ma as u32 / 1000
    }
}

/// Data for a power policy request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PolicyRequestData {
    /// Notify that a device has attached
    NotifyAttached,
    /// Notify that available power for sinking has changed
    NotifySinkPowerCapability(PowerCapability),
    /// Request the given amount of power to source
    RequestSourcePowerCapability(PowerCapability),
    /// Notify that a device has detached
    NotifyDetached,
}

/// Request to the power policy service
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PolicyRequest {
    /// Device that sent this request
    pub id: DeviceId,
    /// Request data
    pub data: PolicyRequestData,
}

/// Data for a power policy response
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PolicyResponseData {
    /// The request was completed successfully
    Complete,
}

/// Response from the power policy service
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PolicyResponse {
    /// Target device
    pub id: DeviceId,
    /// Response data
    pub data: PolicyResponseData,
}

/// Wrapper type to make code cleaner
pub type InternalPolicyResponseData = Result<PolicyResponseData, Error>;

/// Data for a device request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DeviceRequestData {}

/// Request from power policy service to a device
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DeviceRequest {
    /// Target device
    pub id: DeviceId,
    /// Request data
    pub data: DeviceRequestData,
}

/// Data for a device response
pub enum DeviceResponseData {
    /// The request was successful
    Complete,
}

/// Wrapper type to make code cleaner
pub type InternalDeviceResponseData = Result<DeviceResponseData, Error>;

/// Response from a device to the power policy service
pub struct DeviceResponse {
    /// Target device
    pub id: DeviceId,
    /// Response data
    pub data: DeviceResponseData,
}

/// Channel size for device requests
pub const DEVICE_CHANNEL_SIZE: usize = 1;

/// Internal state for a device
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct InternalDeviceState {
    /// Current device state
    pub state: DeviceState,
    /// Source capability of the device
    pub source_capability: Option<PowerCapability>,
    /// Sink capability of the device
    pub sink_capability: Option<PowerCapability>,
}

/// Internal state for the power policy service
pub struct Device {
    /// Intrusive list node
    node: intrusive_list::Node,
    /// Device ID
    id: DeviceId,
    /// Current state of the device
    state: Mutex<NoopRawMutex, InternalDeviceState>,
    /// Channel for requests to the device
    request: Channel<NoopRawMutex, DeviceRequestData, DEVICE_CHANNEL_SIZE>,
    /// Channel for responses from the device
    response: Channel<NoopRawMutex, InternalDeviceResponseData, DEVICE_CHANNEL_SIZE>,
}

impl Device {
    /// Create a new device
    pub fn new(id: DeviceId) -> Self {
        Device {
            node: intrusive_list::Node::uninit(),
            id,
            state: Mutex::new(InternalDeviceState {
                state: DeviceState::Detached,
                source_capability: None,
                sink_capability: None,
            }),
            request: Channel::new(),
            response: Channel::new(),
        }
    }

    /// Sends a request to this device and returns a response
    pub async fn send_request(&self, request: DeviceRequestData) -> Result<DeviceResponseData, Error> {
        self.request.send(request).await;
        self.response.receive().await
    }

    /// Provides exclusive access to the device state
    pub async fn lock_state(&self) -> MutexGuard<'_, NoopRawMutex, InternalDeviceState> {
        self.state.lock().await
    }

    /// Notify the power policy service that this device has attached
    pub async fn notify_attached(&self) -> Result<(), Error> {
        let _ = send_policy_request(self.id, PolicyRequestData::NotifyAttached).await?;
        Ok(())
    }

    /// Notify the power policy service of an updated sink power capability
    pub async fn notify_sink_power_capability(&self, capability: PowerCapability) -> Result<(), Error> {
        let _ = send_policy_request(self.id, PolicyRequestData::NotifySinkPowerCapability(capability)).await?;
        Ok(())
    }

    /// Request the given power from the power policy service
    pub async fn request_source_power_capability(&self, capability: PowerCapability) -> Result<(), Error> {
        let _ = send_policy_request(self.id, PolicyRequestData::RequestSourcePowerCapability(capability)).await?;
        Ok(())
    }

    /// Notify the power policy service that this device has detached
    pub async fn notify_detached(&self) -> Result<(), Error> {
        let _ = send_policy_request(self.id, PolicyRequestData::NotifyDetached).await?;
        Ok(())
    }
}

impl intrusive_list::NodeContainer for Device {
    fn get_node(&self) -> &crate::Node {
        &self.node
    }
}

/// Trait for any container that holds a device
pub trait DeviceContainer {
    /// Get the underlying device struct
    fn get_power_policy_device(&self) -> &Device;
}

impl DeviceContainer for Device {
    fn get_power_policy_device(&self) -> &Device {
        self
    }
}

/// Number of slots for policy requests
const POLICY_CHANNEL_SIZE: usize = 1;

/// Power policy context
struct Context {
    /// Registered devices
    devices: intrusive_list::IntrusiveList,
    /// Policy request
    policy_request: Channel<NoopRawMutex, PolicyRequest, POLICY_CHANNEL_SIZE>,
    /// Policy response
    policy_response: Channel<NoopRawMutex, InternalPolicyResponseData, POLICY_CHANNEL_SIZE>,
}

impl Context {
    fn new() -> Self {
        Context {
            devices: intrusive_list::IntrusiveList::new(),
            policy_request: Channel::new(),
            policy_response: Channel::new(),
        }
    }
}

static CONTEXT: OnceLock<Context> = OnceLock::new();

/// Init power policy service
pub fn init() {
    CONTEXT.get_or_init(Context::new);
}

/// Register a device with the power policy service
pub async fn register_device(device: &'static impl DeviceContainer) -> Result<(), intrusive_list::Error> {
    let device = device.get_power_policy_device();
    if get_device(device.id).await.is_some() {
        return Err(intrusive_list::Error::NodeAlreadyInList);
    }

    CONTEXT.get().await.devices.push(device)
}

/// Find a device by its ID
pub async fn get_device(id: DeviceId) -> Option<&'static Device> {
    for device in &CONTEXT.get().await.devices {
        if let Some(data) = device.data::<Device>() {
            if data.id == id {
                return Some(data);
            }
        } else {
            error!("Non-device located in devices list");
        }
    }

    None
}

/// Convenience function to send a request to a power policy device
async fn send_policy_request(from: DeviceId, request: PolicyRequestData) -> Result<PolicyResponseData, Error> {
    let context = CONTEXT.get().await;
    context
        .policy_request
        .send(PolicyRequest {
            id: from,
            data: request,
        })
        .await;
    context.policy_response.receive().await
}
