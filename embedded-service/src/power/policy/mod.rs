//! Power policy related data structures and messages
pub mod context;
pub mod device;

pub use context::init;

/// Error type
pub enum Error {
    /// The requested device does not exist
    InvalidDevice,
    /// The source request was denied, contains maximum available power
    CannotSource(Option<PowerCapability>),
    /// The sink request was denied, contains maximum available power
    CannotSink(Option<PowerCapability>),
    /// The device is not in the correct state
    InvalidState,
    /// Invalid response
    InvalidResponse,
    /// Bus error
    Bus,
    /// Generic failure
    Failed,
}

/// Device ID new type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DeviceId(pub u8);

/// Amount of power that a device can source or sink
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PowerCapability {
    /// Available voltage in mV
    pub voltage_mv: u16,
    /// Max available current in mA
    pub current_ma: u16,
}

impl PowerCapability {
    /// Calculate maximum power
    pub fn max_power_mw(&self) -> u32 {
        self.voltage_mv as u32 * self.current_ma as u32 / 1000
    }
}
