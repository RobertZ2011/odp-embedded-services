//! Traits and types related to USB operation and alt-modes

use embedded_usb_pd::Error;

use crate::port::{UsbControlConfig, pd::Pd};

/// Trait for controlling USB operation and alt-modes on a port
pub trait Usb: Pd {
    /// Set USB control configuration
    fn set_usb_control(&mut self, config: UsbControlConfig) -> impl Future<Output = Result<(), Error<Self::BusError>>>;

    /// Get USB control configuration
    fn get_usb_control(&mut self) -> impl Future<Output = Result<UsbControlConfig, Error<Self::BusError>>>;
}
