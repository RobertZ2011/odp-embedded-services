//! Traits and types that deal with port state machines

use embedded_usb_pd::Error;

use crate::port::{PdStateMachineConfig, TypeCStateMachineState, pd::Pd};

/// Trait for ports that support Type-C state machine operations
pub trait TypeCStateMachine: Pd {
    /// Set Type-C state-machine configuration
    fn set_type_c_state_machine_config(
        &mut self,
        state: TypeCStateMachineState,
    ) -> impl Future<Output = Result<(), Error<Self::BusError>>>;

    /// Set Type-C state-machine configuration
    fn get_type_c_state_machine_config(
        &mut self,
    ) -> impl Future<Output = Result<TypeCStateMachineState, Error<Self::BusError>>>;
}

/// Trait for ports that support PD state machine operations
pub trait PdStateMachine: Pd {
    /// Set PD state-machine configuration
    fn set_pd_state_machine_config(
        &mut self,
        config: PdStateMachineConfig,
    ) -> impl Future<Output = Result<(), Error<Self::BusError>>>;

    /// Get PD state-machine configuration
    fn get_pd_state_machine_config(
        &mut self,
    ) -> impl Future<Output = Result<PdStateMachineConfig, Error<Self::BusError>>>;
}
