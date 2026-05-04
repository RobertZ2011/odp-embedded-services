//! Module for PD controller related code
use core::future::Future;
use core::num::NonZeroU8;

use embedded_usb_pd::ado::Ado;
use embedded_usb_pd::{LocalPortId, PdError, ucsi::lpm};

use crate::control::dp::{DpConfig, DpStatus};
use crate::control::pd::{PdStateMachineConfig, PortStatus};
use crate::control::power::SystemPowerState;
use crate::control::retimer::RetimerFwUpdateState;
use crate::control::tbt::TbtConfig;
use crate::control::type_c::TypeCStateMachineState;
use crate::control::usb::UsbControlConfig;
use crate::control::vdm::{AttnVdm, OtherVdm, SendVdm};

/// Controller status
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ControllerStatus<'a> {
    /// Current controller mode
    pub mode: &'a str,
    /// True if we did not have to boot from a backup FW bank
    pub valid_fw_bank: bool,
    /// FW version 0
    pub fw_version0: u32,
    /// FW version 1
    pub fw_version1: u32,
}

/// Controller ID
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ControllerId(pub u8);

/// PD controller trait
pub trait Controller {
    /// Returns the port status
    fn get_port_status(&mut self, port: LocalPortId) -> impl Future<Output = Result<PortStatus, PdError>>;

    /// Reset the controller
    fn reset_controller(&mut self) -> impl Future<Output = Result<(), PdError>>;

    /// Returns the retimer fw update state
    fn get_rt_fw_update_status(
        &mut self,
        port: LocalPortId,
    ) -> impl Future<Output = Result<RetimerFwUpdateState, PdError>>;
    /// Set retimer fw update state
    fn set_rt_fw_update_state(&mut self, port: LocalPortId) -> impl Future<Output = Result<(), PdError>>;
    /// Clear retimer fw update state
    fn clear_rt_fw_update_state(&mut self, port: LocalPortId) -> impl Future<Output = Result<(), PdError>>;
    /// Set retimer compliance
    fn set_rt_compliance(&mut self, port: LocalPortId) -> impl Future<Output = Result<(), PdError>>;

    /// Reconfigure the retimer for the given port.
    fn reconfigure_retimer(&mut self, port: LocalPortId) -> impl Future<Output = Result<(), PdError>>;

    /// Clear the dead battery flag for the given port.
    fn clear_dead_battery_flag(&mut self, port: LocalPortId) -> impl Future<Output = Result<(), PdError>>;

    /// Enable or disable sink path
    fn enable_sink_path(&mut self, port: LocalPortId, enable: bool) -> impl Future<Output = Result<(), PdError>>;
    /// Get current controller status
    fn get_controller_status(&mut self) -> impl Future<Output = Result<ControllerStatus<'static>, PdError>>;
    /// Get current PD alert
    fn get_pd_alert(&mut self, port: LocalPortId) -> impl Future<Output = Result<Option<Ado>, PdError>>;
    /// Set the maximum sink voltage for the given port
    ///
    /// This may trigger a renegotiation
    fn set_max_sink_voltage(
        &mut self,
        port: LocalPortId,
        voltage_mv: Option<u16>,
    ) -> impl Future<Output = Result<(), PdError>>;
    /// Set port unconstrained status
    fn set_unconstrained_power(
        &mut self,
        port: LocalPortId,
        unconstrained: bool,
    ) -> impl Future<Output = Result<(), PdError>>;

    /// Get the Rx Other VDM data for the given port
    fn get_other_vdm(&mut self, port: LocalPortId) -> impl Future<Output = Result<OtherVdm, PdError>>;
    /// Get the Rx Attention VDM data for the given port
    fn get_attn_vdm(&mut self, port: LocalPortId) -> impl Future<Output = Result<AttnVdm, PdError>>;
    /// Send a VDM to the given port
    fn send_vdm(&mut self, port: LocalPortId, tx_vdm: SendVdm) -> impl Future<Output = Result<(), PdError>>;

    /// Set USB control configuration for the given port
    fn set_usb_control(
        &mut self,
        port: LocalPortId,
        config: UsbControlConfig,
    ) -> impl Future<Output = Result<(), PdError>>;

    /// Get DisplayPort status for the given port
    fn get_dp_status(&mut self, port: LocalPortId) -> impl Future<Output = Result<DpStatus, PdError>>;
    /// Set DisplayPort configuration for the given port
    fn set_dp_config(&mut self, port: LocalPortId, config: DpConfig) -> impl Future<Output = Result<(), PdError>>;
    /// Execute PD Data Reset for the given port
    fn execute_drst(&mut self, port: LocalPortId) -> impl Future<Output = Result<(), PdError>>;

    /// Set Thunderbolt configuration for the given port
    fn set_tbt_config(&mut self, port: LocalPortId, config: TbtConfig) -> impl Future<Output = Result<(), PdError>>;

    /// Set PD state-machine configuration for the given port
    fn set_pd_state_machine_config(
        &mut self,
        port: LocalPortId,
        config: PdStateMachineConfig,
    ) -> impl Future<Output = Result<(), PdError>>;

    /// Set Type-C state-machine configuration for the given port
    fn set_type_c_state_machine_config(
        &mut self,
        port: LocalPortId,
        state: TypeCStateMachineState,
    ) -> impl Future<Output = Result<(), PdError>>;

    /// Execute the given UCSI command
    fn execute_ucsi_command(
        &mut self,
        command: lpm::LocalCommand,
    ) -> impl Future<Output = Result<Option<lpm::ResponseData>, PdError>>;

    /// Execute an electrical disconnect on the given port, if supported by the controller.
    ///
    /// If `reconnect_time_s` is provided, the controller should automatically reconnect the port after the specified time
    /// has elapsed. If `reconnect_time_s` is [`None`], the port should remain disconnected until manually reconnected.
    fn execute_electrical_disconnect(
        &mut self,
        port: LocalPortId,
        reconnect_time_s: Option<NonZeroU8>,
    ) -> impl Future<Output = Result<(), PdError>>;

    /// Set the system power state on the given port.
    ///
    /// This notifies the PD controller of the current system power state,
    /// which triggers Application Configuration updates (e.g., crossbar reconfiguration).
    fn set_power_state(
        &mut self,
        port: LocalPortId,
        state: SystemPowerState,
    ) -> impl Future<Output = Result<(), PdError>>;
}
