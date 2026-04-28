//! Module contain power-policy related message handling
use crate::wrapper::config::UnconstrainedSink;
use power_policy_interface::capability::{ConsumerPowerCapability, ProviderPowerCapability, PsuType};

use super::*;

impl<'device, M: RawMutex, D: Lockable, S: event::Sender<power_policy_interface::psu::event::EventData>>
    ControllerWrapper<'device, M, D, S>
where
    D::Inner: Controller,
{
    /// Handle a new contract as consumer
    pub(super) async fn process_new_consumer_contract(
        &self,
        port_state: &mut PortState<S>,
        psu_state: &mut power_policy_interface::psu::State,
        status: &PortStatus,
    ) -> Result<(), Error<<D::Inner as Controller>::BusError>> {
        info!("Process new consumer contract");
        let available_sink_contract = status.available_sink_contract.map(|c| {
            let mut c: ConsumerPowerCapability = c.into();
            let unconstrained = match self.config.unconstrained_sink {
                UnconstrainedSink::Auto => status.unconstrained_power,
                UnconstrainedSink::PowerThresholdMilliwatts(threshold) => c.capability.max_power_mw() >= threshold,
                UnconstrainedSink::Never => false,
            };
            c.flags.set_unconstrained_power(unconstrained);
            c.flags.set_psu_type(PsuType::TypeC);
            c
        });

        if let Err(e) = psu_state.update_consumer_power_capability(available_sink_contract) {
            error!("Failed to update consumer power capability: {:?}", e);
            return Err(Error::Pd(PdError::Failed));
        }
        port_state
            .power_policy_sender
            .send(power_policy_interface::psu::event::EventData::UpdatedConsumerCapability(available_sink_contract))
            .await;
        Ok(())
    }

    /// Handle a new contract as provider
    pub(super) async fn process_new_provider_contract(
        &self,
        port_state: &mut PortState<S>,
        psu_state: &mut power_policy_interface::psu::State,
        status: &PortStatus,
    ) -> Result<(), Error<<D::Inner as Controller>::BusError>> {
        info!("Process New provider contract");
        let capability = status.available_source_contract.map(|caps| {
            let mut caps = ProviderPowerCapability::from(caps);
            caps.flags.set_psu_type(PsuType::TypeC);
            caps
        });
        if let Err(e) = psu_state.update_requested_provider_power_capability(capability) {
            error!("Failed to update requested provider power capability: {:?}", e);
            return Err(Error::Pd(PdError::Failed));
        }
        port_state
            .power_policy_sender
            .send(power_policy_interface::psu::event::EventData::RequestedProviderCapability(capability))
            .await;
        Ok(())
    }
}
