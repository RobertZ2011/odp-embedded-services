//! Module for general firmware updates
use core::future::Future;

/// FW update error kind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ErrorKind {
    /// The device is not in FW update mode
    InvalidMode,
    /// FW update Content was invalid
    InvalidContent,
    /// Full FW update contents were not received
    Incomplete,
    /// The operation did not complete in the expected time
    Timeout,
    /// Error communicating with the device due to a bus error
    Bus,
    /// Generic failure
    Failed,
}

/// Convenient error type for devices that are on a bus
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error<B> {
    /// Bus error
    Bus(B),
    /// Generic error
    General(ErrorKind),
}

/// General firmware update trait
pub trait FwUpdate {
    /// Error type
    type BusError;

    /// Get the current firmware version
    fn get_active_fw_version(&self) -> impl Future<Output = Result<u32, Error<Self::BusError>>>;

    /// Begin a new firmware update
    fn start_fw_update(&mut self) -> impl Future<Output = Result<(), Error<Self::BusError>>>;

    /// Terminate the firmware update after a failure
    fn abort_fw_update(&mut self) -> impl Future<Output = Result<(), Error<Self::BusError>>>;

    /// Finalize the firmware update after all contents have been sent
    fn finalize_fw_update(&mut self) -> impl Future<Output = Result<(), Error<Self::BusError>>>;

    /// Supply firmware update contents
    fn write_fw_contents(&mut self, offset: usize, data: &[u8]) -> impl Future<Output = Result<(), Error<Self::BusError>>>;
}
