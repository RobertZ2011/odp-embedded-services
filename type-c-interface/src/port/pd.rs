//! Module for core PD trait

use embedded_services::bus_error::BusError;
use embedded_usb_pd::{Error, ado::Ado};

use crate::port::{AttnVdm, OtherVdm, PortStatus, SendVdm};

/// Core PD trait containing base functionality from the PD spec.
pub trait Pd: BusError {
    /// Returns the port status
    fn get_port_status(&mut self) -> impl Future<Output = Result<PortStatus, Error<Self::BusError>>>;

    /// Clear the dead battery flag
    fn clear_dead_battery_flag(&mut self) -> impl Future<Output = Result<(), Error<Self::BusError>>>;

    /// Enable or disable sink path
    fn enable_sink_path(&mut self, enable: bool) -> impl Future<Output = Result<(), Error<Self::BusError>>>;

    /// Get current PD alert
    fn get_pd_alert(&mut self) -> impl Future<Output = Result<Option<Ado>, Error<Self::BusError>>>;

    /// Set port unconstrained status
    fn set_unconstrained_power(
        &mut self,
        unconstrained: bool,
    ) -> impl Future<Output = Result<(), Error<Self::BusError>>>;

    /// Get the Rx Other VDM data
    fn get_other_vdm(&mut self) -> impl Future<Output = Result<OtherVdm, Error<Self::BusError>>>;
    /// Get the Rx Attention VDM data
    fn get_attn_vdm(&mut self) -> impl Future<Output = Result<AttnVdm, Error<Self::BusError>>>;
    /// Send a VDM
    fn send_vdm(&mut self, tx_vdm: SendVdm) -> impl Future<Output = Result<(), Error<Self::BusError>>>;

    /// Execute PD Data Reset
    fn execute_drst(&mut self) -> impl Future<Output = Result<(), Error<Self::BusError>>>;
}
