//! Power policy actions
//! This modules contains wrapper structs that use type states to enforce the valid actions for each device state
use super::device::StateKind;

mod device;
mod policy;

pub use device::*;
pub use policy::*;

/// Trait to provide the kind of a state type
pub trait Kind {
    /// Return the kind of a state type
    fn kind() -> StateKind;
}

/// State type for a detached device
pub struct Detached;
impl Kind for Detached {
    fn kind() -> StateKind {
        StateKind::Detached
    }
}

/// State type for an attached device
pub struct Attached;
impl Kind for Attached {
    fn kind() -> StateKind {
        StateKind::Attached
    }
}

/// State type for a device that is sourcing power
pub struct Source;
impl Kind for Source {
    fn kind() -> StateKind {
        StateKind::Source
    }
}

/// State type for a device that is sinking power
pub struct Sink;
impl Kind for Sink {
    fn kind() -> StateKind {
        StateKind::Sink
    }
}
