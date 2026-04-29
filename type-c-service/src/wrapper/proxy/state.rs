use embassy_time::Instant;
use type_c_interface::port::event::PortStatusEventBitfield;

/// State shared between the port and event receiver
#[derive(Copy, Clone)]
pub struct SharedState {
    /// Sink ready timeout
    pub(crate) sink_ready_timeout: Option<Instant>,
    /// Pending software status event
    pub(crate) sw_status_event: PortStatusEventBitfield,
}

impl SharedState {
    /// Create a new instance with default values
    pub fn new() -> Self {
        Self {
            sink_ready_timeout: None,
            sw_status_event: PortStatusEventBitfield::none(),
        }
    }
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}
