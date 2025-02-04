//! Context for any power policy implementations
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;

use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel, once_lock::OnceLock};

use crate::{error, intrusive_list};

use super::device::*;
use super::{DeviceId, Error, PowerCapability};

/// Number of slots for policy requests
const POLICY_CHANNEL_SIZE: usize = 1;

/// Data for a power policy request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PolicyRequestData {
    /// Notify that a device has attached
    NotifyAttached,
    /// Notify that available power for sinking has changed
    NotifySinkPowerCapability(Option<PowerCapability>),
    /// Request the given amount of power to source
    RequestSourcePowerCapability(PowerCapability),
    /// Notify that a device cannot source or sink power anymore
    NotifyDisconnect,
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
        Self {
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
    if get_device(device.id()).await.is_some() {
        return Err(intrusive_list::Error::NodeAlreadyInList);
    }

    CONTEXT.get().await.devices.push(device)
}

/// Find a device by its ID
async fn get_device(id: DeviceId) -> Option<&'static Device> {
    for device in &CONTEXT.get().await.devices {
        if let Some(data) = device.data::<Device>() {
            if data.id() == id {
                return Some(data);
            }
        } else {
            error!("Non-device located in devices list");
        }
    }

    None
}

/// Convenience function to send a request to a power policy device
pub(super) async fn send_policy_request(
    from: DeviceId,
    request: PolicyRequestData,
) -> Result<PolicyResponseData, Error> {
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

/// Singleton struct to give access to the power policy context
pub struct ContextToken(());

impl ContextToken {
    /// Create a new context token, panicking if this function has been called before
    pub fn create() -> Self {
        static INIT: AtomicBool = AtomicBool::new(false);
        if INIT.load(Ordering::SeqCst) {
            panic!("Request listener already initialized");
        }

        INIT.store(true, Ordering::SeqCst);
        ContextToken(())
    }

    /// Wait for a power policy request
    pub async fn wait_request(&mut self) -> PolicyRequest {
        CONTEXT.get().await.policy_request.receive().await
    }

    /// Get a device by its ID
    pub async fn get_device(&mut self, id: DeviceId) -> Option<&'static Device> {
        get_device(id).await
    }

    /// Provides access to the device list
    pub async fn devices(&mut self) -> &intrusive_list::IntrusiveList {
        &CONTEXT.get().await.devices
    }
}
