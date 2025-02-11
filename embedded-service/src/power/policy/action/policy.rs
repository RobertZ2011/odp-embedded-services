use super::*;
use crate::info;
use crate::power::policy::{device, Error, PowerCapability};

/// Policy state machine control
pub struct Policy<'a, S: Kind> {
    device: &'a device::Device,
    _state: core::marker::PhantomData<S>,
}

impl<'a, S: Kind> Policy<'a, S> {
    /// Create a new state machine
    pub(crate) fn new(device: &'a device::Device) -> Self {
        Self {
            device,
            _state: core::marker::PhantomData,
        }
    }

    async fn disconnect_internal(&self) -> Result<(), Error> {
        info!("Device {} got disconnect request", self.device.id().0);
        self.device
            .execute_device_request(device::RequestData::Disconnect)
            .await?
            .complete_or_err()
    }
}

// The policy can do nothing when no device is attached
impl Policy<'_, Detached> {}

impl<'a> Policy<'a, Attached> {
    /// Connect this device as a sink
    pub async fn connect_sink(self, capability: PowerCapability) -> Result<Policy<'a, Sink>, Error> {
        info!("Device {} connecting sink", self.device.id().0);

        self.device
            .execute_device_request(device::RequestData::ConnectSink(capability))
            .await?
            .complete_or_err()?;

        self.device.set_state(device::State::Sink(capability)).await;
        Ok(Policy::new(self.device))
    }

    /// Connect this device as a source
    pub async fn connect_source(self, capability: PowerCapability) -> Result<Policy<'a, Source>, Error> {
        info!("Device {} connecting source", self.device.id().0);

        self.device
            .execute_device_request(device::RequestData::ConnectSource(capability))
            .await?
            .complete_or_err()?;

        self.device.set_state(device::State::Source(capability)).await;
        Ok(Policy::new(self.device))
    }
}

impl<'a> Policy<'a, Sink> {
    /// Disconnect this device
    pub async fn disconnect(self) -> Result<Policy<'a, Attached>, Error> {
        self.disconnect_internal().await?;
        Ok(Policy::new(self.device))
    }
}

impl<'a> Policy<'a, Source> {
    /// Disconnect this device
    pub async fn disconnect(self) -> Result<Policy<'a, Attached>, Error> {
        self.disconnect_internal().await?;
        Ok(Policy::new(self.device))
    }
}
