//! Traits and types related to DisplayPort alternate mode

use embedded_usb_pd::Error;

use crate::port::{DpConfig, DpStatus, pd::Pd};

/// Trait for ports that support DisplayPort alternate mode operations
pub trait DisplayPort: Pd {
    /// Get DisplayPort status
    fn get_dp_status(&mut self) -> impl Future<Output = Result<DpStatus, Error<Self::BusError>>>;
    /// Set DisplayPort configuration
    fn set_dp_config(&mut self, config: DpConfig) -> impl Future<Output = Result<(), Error<Self::BusError>>>;
    /// Get DisplayPort configuration
    fn get_dp_config(&mut self) -> impl Future<Output = Result<DpConfig, Error<Self::BusError>>>;
}
