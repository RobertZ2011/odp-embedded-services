//! Port event types

/// Top-level port event type
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum Event {
    /// Port event
    PortEvent(type_c_interface::port::event::PortEvent),
}
