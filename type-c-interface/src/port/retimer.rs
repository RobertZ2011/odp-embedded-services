//! Module for retimer related traits and types

use embedded_services::bus_error::BusError;
use embedded_usb_pd::Error;

use crate::port::RetimerFwUpdateState;

/// Retimer trait
pub trait Retimer: BusError {
    /// Returns the retimer fw update state
    fn get_rt_fw_update_status(&mut self) -> impl Future<Output = Result<RetimerFwUpdateState, Error<Self::BusError>>>;
    /// Set retimer fw update state
    fn set_rt_fw_update_state(&mut self) -> impl Future<Output = Result<(), Error<Self::BusError>>>;
    /// Clear retimer fw update state
    fn clear_rt_fw_update_state(&mut self) -> impl Future<Output = Result<(), Error<Self::BusError>>>;
    /// Set retimer compliance
    fn set_rt_compliance(&mut self) -> impl Future<Output = Result<(), Error<Self::BusError>>>;

    /// Reconfigure the retimer for the given port.
    fn reconfigure_retimer(&mut self) -> impl Future<Output = Result<(), Error<Self::BusError>>>;
}
