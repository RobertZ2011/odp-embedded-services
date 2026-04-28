//! Port event types

/// Top-level port event type
pub enum Event {
    /// Port status changed
    PortEvent(type_c_interface::port::event::PortEvent),
}
