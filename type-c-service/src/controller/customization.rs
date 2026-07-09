//! Trait and related code to inject OEM-defined logic into the controller manager struct.

use core::future::ready;

use embedded_usb_pd::PdError;
use power_policy_interface::capability::ConsumerPowerCapability;
use type_c_interface::control::pd::PortStatus;

/// Controller management customization trait
pub trait Customization {
    fn update_consumer_capability(
        &mut self,
        port_status: &PortStatus,
        capability: ConsumerPowerCapability,
    ) -> impl Future<Output = Result<ConsumerPowerCapability, PdError>>;
}

/// Unconstrained behavior for sink role
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum UnconstrainedSink {
    /// Automatically signal unconstrained power based on unconstrained bit in PDO
    #[default]
    Auto,
    /// Automatically signal unconstrained power for any sink that meets a power threshold in mW
    PowerThresholdMilliwatts(u32),
    /// Never signal unconstrained power
    Never,
}

impl UnconstrainedSink {
    pub fn set_consumer_flags(&self, port_status: &PortStatus, capability: &mut ConsumerPowerCapability) {
        capability.flags.set_unconstrained_power(match self {
            UnconstrainedSink::Auto => port_status.unconstrained_power,
            UnconstrainedSink::PowerThresholdMilliwatts(threshold) => {
                capability.capability.max_power_mw() >= *threshold
            }
            UnconstrainedSink::Never => false,
        });
    }
}

/// Default behavior when no OEM-specific logic is required
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct DefaultCustomization {
    /// Unconstrained sink behavior
    pub unconstrained: UnconstrainedSink,
}

impl Customization for DefaultCustomization {
    fn update_consumer_capability(
        &mut self,
        port_status: &PortStatus,
        capability: ConsumerPowerCapability,
    ) -> impl Future<Output = Result<ConsumerPowerCapability, PdError>> {
        let mut capability = capability;
        self.unconstrained.set_consumer_flags(port_status, &mut capability);
        ready(Ok(capability))
    }
}
