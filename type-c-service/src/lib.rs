#![no_std]
pub mod driver;
mod task;
pub mod wrapper;

use embedded_services::type_c::event::{PortEventVariant, PortNotification, PortPendingIter};
pub use task::task;

/// Iterator to contain state for iterating over all pending port events
pub struct PortEventStreamer {
    /// Iterator over pending ports
    pending_iter: PortPendingIter,
    /// Notification to be streamed
    pending_notifitications: Option<PortNotification>,
}

impl PortEventStreamer {
    pub fn new(pending_iter: PortPendingIter) -> Self {
        Self {
            pending_iter,
            pending_notifitications: None,
        }
    }
}

impl PortEventStreamer {
    /// Get the next port event
    pub fn next(&mut self) -> Option<PortEventVariant> {
        if let Some(notification) = self.pending_notifitications.take() {
            return Some(notification);
        }

        if let Some(port) = self.pending_iter.next() {
            self.pending_notifitications = Some(PortNotification::new(port));
            self.pending_notifitications.clone()
        } else {
            None
        }
    }
}
