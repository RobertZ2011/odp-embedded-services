//! Device struct and methods
use core::ops::DerefMut;

use embassy_sync::{
    blocking_mutex::raw::NoopRawMutex,
    channel::Channel,
    mutex::{Mutex, MutexGuard},
};

use crate::{info, intrusive_list, warn};

use super::context::*;
use super::{DeviceId, Error, PowerCapability};

/// Current state of the attached power device
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum State {
    /// Device is attached, but is not currently sourcing or sinking power
    Attached,
    /// Device is attached and is currently sourcing power
    Source(PowerCapability),
    /// Device is attached and is currently sinking power
    Sink(PowerCapability),
    /// No device attached
    Detached,
}

/// Internal device state for power policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
struct InternalState {
    /// Current state of the device
    pub state: State,
    /// Current sink capability
    pub sink_capability: Option<PowerCapability>,
}

/// Data for a device request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DeviceRequestData {
    /// Start sinking on this port
    ConnectSink(PowerCapability),
    /// Start sourcing on this port
    ConnectSource(PowerCapability),
    /// Stop sourcing or sinking on this port
    Disconnect,
}

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

/// Internal state for the power policy service
pub struct Device {
    /// Intrusive list node
    node: intrusive_list::Node,
    /// Device ID
    id: DeviceId,
    /// Current state of the device
    state: Mutex<NoopRawMutex, InternalState>,
    /// Channel for requests to the device
    request: Channel<NoopRawMutex, DeviceRequestData, DEVICE_CHANNEL_SIZE>,
    /// Channel for responses from the device
    response: Channel<NoopRawMutex, InternalDeviceResponseData, DEVICE_CHANNEL_SIZE>,
}

impl Device {
    /// Create a new device
    pub fn new(id: DeviceId) -> Self {
        Self {
            node: intrusive_list::Node::uninit(),
            id,
            state: Mutex::new(InternalState {
                state: State::Detached,
                sink_capability: None,
            }),
            request: Channel::new(),
            response: Channel::new(),
        }
    }

    /// Get the device ID
    pub fn id(&self) -> DeviceId {
        self.id
    }

    /// Sends a request to this device and returns a response
    async fn execute_device_request(&self, request: DeviceRequestData) -> Result<DeviceResponseData, Error> {
        self.request.send(request).await;
        self.response.receive().await
    }

    /// Provides exclusive access to the device state
    async fn lock_state(&self) -> MutexGuard<'_, NoopRawMutex, InternalState> {
        self.state.lock().await
    }

    /// Returns the current state of the device
    pub async fn state(&self) -> State {
        self.lock_state().await.state
    }

    /// Returns the current sink capability of the device
    pub async fn sink_capability(&self) -> Option<PowerCapability> {
        self.lock_state().await.sink_capability
    }

    /// Notify the power policy service that this device has attached
    pub async fn policy_notify_attached(&self) -> Result<(), Error> {
        info!("Received attach from device {}", self.id().0);

        {
            let mut lock = self.lock_state().await;
            let state = lock.deref_mut();
            if state.state != State::Detached {
                warn!("Received attach request for device that is not detached");
            }

            state.state = State::Attached;
        }

        let _ = send_policy_request(self.id, PolicyRequestData::NotifyAttached).await?;
        Ok(())
    }

    /// Notify the power policy service of an updated sink power capability
    pub async fn policy_notify_sink_power_capability(&self, capability: Option<PowerCapability>) -> Result<(), Error> {
        info!("Device {} sink capability updated {:#?}", self.id().0, capability);

        {
            let mut lock = self.lock_state().await;
            let state = lock.deref_mut();
            if state.state == State::Detached {
                warn!("Received sink capability for device that is not attached");
            }

            state.sink_capability = capability;
        }
        let _ = send_policy_request(self.id, PolicyRequestData::NotifySinkPowerCapability(capability)).await?;
        Ok(())
    }

    /// Request the given power from the power policy service
    pub async fn policy_request_source_power_capability(&self, capability: PowerCapability) -> Result<(), Error> {
        info!("Request source from device {}, {:#?}", self.id.0, capability);

        {
            let mut lock = self.lock_state().await;
            let state = lock.deref_mut();
            if state.state != State::Attached {
                warn!("Received request source power capability for device that is not attached");
            }
        }

        let _ = send_policy_request(self.id, PolicyRequestData::RequestSourcePowerCapability(capability)).await?;
        Ok(())
    }

    /// Notify the power policy service that this device cannot source or sink power anymore
    pub async fn policy_notify_disconnect(&self) -> Result<(), Error> {
        info!("Received disconnect from device {}", self.id.0);

        {
            let mut lock = self.lock_state().await;
            let state = lock.deref_mut();
            if !matches!(state.state, State::Sink(_)) && !matches!(state.state, State::Source(_)) {
                warn!("Received disconnect request for device that is not attached");
            }

            state.state = State::Attached;
        }

        let _ = send_policy_request(self.id, PolicyRequestData::NotifyDisconnect).await?;
        Ok(())
    }

    /// Notify the power policy service that this device has detached
    pub async fn policy_notify_detached(&self) -> Result<(), Error> {
        info!("Received detach from device {}", self.id.0);

        {
            let mut lock = self.lock_state().await;
            let state = lock.deref_mut();
            if state.state == State::Detached {
                warn!("Received detach request for device that is not attached");
            }

            state.state = State::Detached;
        }

        let _ = send_policy_request(self.id, PolicyRequestData::NotifyDetached).await?;
        Ok(())
    }

    /// Connect this device as a sink
    pub async fn device_connect_sink(&self, capability: PowerCapability) -> Result<(), Error> {
        info!("Device {} connecting sink", self.id.0);

        let _ = self
            .execute_device_request(DeviceRequestData::ConnectSink(capability))
            .await?;

        {
            let mut lock = self.lock_state().await;
            let state = lock.deref_mut();
            if state.state != State::Attached {
                warn!("Received connect sink request for device that is not attached");
            }

            state.state = State::Source(capability);
        }
        Ok(())
    }

    /// Connect this device as a source
    pub async fn device_connect_source(&self, capability: PowerCapability) -> Result<(), Error> {
        info!("Device {} connecting source", self.id.0);

        let _ = self
            .execute_device_request(DeviceRequestData::ConnectSource(capability))
            .await?;

        {
            let mut lock = self.lock_state().await;
            let state = lock.deref_mut();
            if state.state != State::Detached {
                warn!("Received connect source request for device that is not attached");
            }

            state.state = State::Source(capability);
        }
        Ok(())
    }

    /// Disconnect this device
    pub async fn device_disconnect(&self) -> Result<(), Error> {
        info!("Device {} disconnecting", self.id.0);

        let _ = self.execute_device_request(DeviceRequestData::Disconnect).await?;

        {
            let mut lock = self.lock_state().await;
            let state = lock.deref_mut();

            if !matches!(state.state, State::Sink(_)) && !matches!(state.state, State::Source(_)) {
                warn!("Received disconnect request for device that is not sourcing or sinking");
            }

            state.state = State::Attached;
        }
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
