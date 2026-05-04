//! Module for PD controller related code
//! use embedded_usb_pd::{LocalPortId, PdError};

use embedded_usb_pd::PdError;

pub mod electrical_disconnect;
pub mod max_sink_voltage;
pub mod pd;
pub mod power;
pub mod retimer;
pub mod type_c;

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
    /// Reset the controller
    fn reset_controller(&mut self) -> impl Future<Output = Result<(), PdError>>;

    /// Get current controller status
    fn get_controller_status(&mut self) -> impl Future<Output = Result<ControllerStatus<'static>, PdError>>;
}
