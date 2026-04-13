//! Module for core PD trait
use embedded_usb_pd::{PdError, ado::Ado};

use crate::port::{AttnVdm, OtherVdm, PortStatus, SendVdm};

/// Core PD trait containing base functionality from the PD spec.
pub trait Pd {
    /// Returns the port status
    fn get_port_status(&mut self) -> impl Future<Output = Result<PortStatus, PdError>>;

    /// Clear the dead battery flag
    fn clear_dead_battery_flag(&mut self) -> impl Future<Output = Result<(), PdError>>;

    /// Enable or disable sink path
    fn enable_sink_path(&mut self, enable: bool) -> impl Future<Output = Result<(), PdError>>;

    /// Get current PD alert
    fn get_pd_alert(&mut self) -> impl Future<Output = Result<Option<Ado>, PdError>>;

    /// Set port unconstrained status
    fn set_unconstrained_power(&mut self, unconstrained: bool) -> impl Future<Output = Result<(), PdError>>;

    /// Get the Rx Other VDM data
    fn get_other_vdm(&mut self) -> impl Future<Output = Result<OtherVdm, PdError>>;
    /// Get the Rx Attention VDM data
    fn get_attn_vdm(&mut self) -> impl Future<Output = Result<AttnVdm, PdError>>;
    /// Send a VDM
    fn send_vdm(&mut self, tx_vdm: SendVdm) -> impl Future<Output = Result<(), PdError>>;

    /// Execute PD Data Reset
    fn execute_drst(&mut self) -> impl Future<Output = Result<(), PdError>>;
}
