//! Module for traits related to bus errors

/// Super trait for traits that need a bus-specific error type.
pub trait BusError {
    /// Type of error returned by the bus
    type BusError;
}
