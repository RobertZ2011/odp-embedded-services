use embedded_services::{error, info, named::Named, sync::Lockable};
use embedded_usb_pd::LocalPortId;
use power_policy_interface::psu::Psu;
use type_c_interface::port::Controller;

use crate::util::power_policy_error_from_pd_bus_error;

pub struct PowerProxyDevice<'device, C: Lockable<Inner: Controller>> {
    /// Local port
    port: LocalPortId,
    /// Controller
    controller: &'device C,
    /// Per-port PSU state
    pub(crate) psu_state: power_policy_interface::psu::State,
    name: &'static str,
}

impl<'device, C: Lockable<Inner: Controller>> PowerProxyDevice<'device, C> {
    pub fn new(name: &'static str, port: LocalPortId, controller: &'device C) -> Self {
        Self {
            name,
            controller,
            port,
            psu_state: power_policy_interface::psu::State::default(),
        }
    }
}

impl<'device, C: Lockable<Inner: Controller>> Psu for PowerProxyDevice<'device, C> {
    async fn disconnect(&mut self) -> Result<(), power_policy_interface::psu::Error> {
        self.controller
            .lock()
            .await
            .enable_sink_path(self.port, false)
            .await
            .map_err(|e| {
                error!("({}): Error disabling sink path", self.name);
                power_policy_error_from_pd_bus_error(e)
            })?;
        self.psu_state.disconnect(false)
    }

    async fn connect_provider(
        &mut self,
        capability: power_policy_interface::capability::ProviderPowerCapability,
    ) -> Result<(), power_policy_interface::psu::Error> {
        info!("({}): Connect as provider: {:#?}", self.name, capability);
        // TODO: Implement controller over provider enablement
        self.psu_state.connect_provider(capability).inspect_err(|e| {
            error!("({}): Failed to transition to provider state: {:#?}", self.name, e);
        })
    }

    async fn connect_consumer(
        &mut self,
        capability: power_policy_interface::capability::ConsumerPowerCapability,
    ) -> Result<(), power_policy_interface::psu::Error> {
        info!(
            "({}): Connect as consumer: {:?}, enable input switch",
            self.name, capability
        );
        self.controller
            .lock()
            .await
            .enable_sink_path(self.port, true)
            .await
            .map_err(|e| {
                error!("({}): Error enabling sink path", self.name);
                power_policy_error_from_pd_bus_error(e)
            })?;
        self.psu_state.connect_consumer(capability)
    }

    fn state(&self) -> &power_policy_interface::psu::State {
        &self.psu_state
    }

    fn state_mut(&mut self) -> &mut power_policy_interface::psu::State {
        &mut self.psu_state
    }
}

impl<'device, C: Lockable<Inner: Controller>> Named for PowerProxyDevice<'device, C> {
    fn name(&self) -> &'static str {
        self.name
    }
}
