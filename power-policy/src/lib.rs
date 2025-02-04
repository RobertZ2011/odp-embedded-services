#![no_std]

use core::ops::DerefMut;

use embedded_services::power::policy::context::*;
use embedded_services::power::policy::device::Device;
use embedded_services::{info, power::policy::*, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SinkState {
    device_id: DeviceId,
    power_capability: PowerCapability,
}

impl PartialOrd for SinkState {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(
            self.power_capability
                .max_power_mw()
                .cmp(&other.power_capability.max_power_mw()),
        )
    }
}

impl Ord for SinkState {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.power_capability
            .max_power_mw()
            .cmp(&other.power_capability.max_power_mw())
    }
}

struct PowerPolicy {
    context: ContextToken,
    /// Current sink state, if any
    current_sink_state: Option<SinkState>,
}

impl PowerPolicy {
    pub fn create() -> Self {
        Self {
            context: ContextToken::create(),
            current_sink_state: None,
        }
    }

    async fn process_notify_attach(&mut self, _device: &Device) -> Result<(), Error> {}

    async fn process_notify_detach(&mut self, _device: &Device) -> Result<(), Error> {
        self.update_sink().await
    }

    async fn process_notify_sink_power_capability(&mut self, _device: &Device) -> Result<(), Error> {
        self.update_sink().await
    }

    async fn update_sink(&mut self) -> Result<(), Error> {
        let current_best_sink = self.current_sink_state;
        let mut best_sink = self.current_sink_state;

        for node in self.context.devices().await {
            let device = node.data::<Device>().ok_or(Error::InvalidDevice)?;
        }

        Ok(())
    }

    async fn process_request(&mut self) -> Result<(), Error> {
        let request = self.context.wait_request().await;
        let device = self.context.get_device(request.id).await.ok_or(Error::InvalidDevice)?;

        match request.data {
            PolicyRequestData::NotifyAttached => self.process_notify_attach(device).await?,
            PolicyRequestData::NotifyDetached => self.process_notify_detach(request.id, device_state).await?,
            PolicyRequestData::NotifySinkPowerCapability(capability) => {
                self.process_sink_power_capability(request.id, capability, device_state)
                    .await?
            }
            PolicyRequestData::RequestSourcePowerCapability(capability) => {
                self.process_source_power_capability(request.id, capability, device_state)
                    .await?
            }
        }

        Ok(())
    }
}
