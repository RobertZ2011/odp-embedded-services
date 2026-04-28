//! Module for power policy related functionality
use embedded_services::{error, info, sync::Lockable};
use power_policy_interface::{
    capability::{ConsumerPowerCapability, ProviderPowerCapability},
    psu::{Error, Psu, State},
};
use type_c_interface::port::Controller;

use crate::util::power_policy_error_from_pd_bus_error;

use super::*;

impl<'device, C: Lockable<Inner: Controller>> Psu for PowerProxyDevice<'device, C> {
    async fn disconnect(&mut self) -> Result<(), Error> {
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

    async fn connect_provider(&mut self, capability: ProviderPowerCapability) -> Result<(), Error> {
        info!("({}): Connect as provider: {:#?}", self.name, capability);
        // TODO: Implement controller over provider enablement
        self.psu_state.connect_provider(capability).inspect_err(|e| {
            error!("({}): Failed to transition to provider state: {:#?}", self.name, e);
        })
    }

    async fn connect_consumer(&mut self, capability: ConsumerPowerCapability) -> Result<(), Error> {
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

    fn state(&self) -> &State {
        &self.psu_state
    }

    fn state_mut(&mut self) -> &mut State {
        &mut self.psu_state
    }
}
