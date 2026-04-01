//! Traits and types related to Thunderbolt alternate mode

use embedded_usb_pd::Error;

use crate::port::{TbtConfig, pd::Pd};

/// Trait for ports that support Thunderbolt alternate mode operations
pub trait Thunderbolt: Pd {
    /// Set Thunderbolt configuration
    fn set_tbt_config(&mut self, config: TbtConfig) -> impl Future<Output = Result<(), Error<Self::BusError>>>;
    /// Get Thunderbolt configuration
    fn get_tbt_config(&mut self) -> impl Future<Output = Result<TbtConfig, Error<Self::BusError>>>;
}
