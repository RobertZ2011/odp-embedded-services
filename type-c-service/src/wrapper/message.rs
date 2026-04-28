//! [`crate::wrapper::ControllerWrapper`] message types
use embedded_usb_pd::{LocalPortId, ado::Ado};

use type_c_interface::{
    port::event::PortStatusEventBitfield,
    port::{DpStatus, PortStatus},
};

/// Port event
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LocalPortEvent {
    /// Port ID
    pub port: LocalPortId,
    /// Port event
    pub event: type_c_interface::port::event::PortEvent,
}

/// Wrapper events
pub enum Event {
    /// Port status changed
    PortEvent(LocalPortEvent),
}

/// Port status changed output data
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct OutputPortStatusChanged {
    /// Port ID
    pub port: LocalPortId,
    /// Status changed event
    pub status_event: PortStatusEventBitfield,
    /// Port status
    pub status: PortStatus,
}

/// PD alert output data
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct OutputPdAlert {
    /// Port ID
    pub port: LocalPortId,
    /// ADO data
    pub ado: Ado,
}

pub mod vdm {
    //! Events and output for vendor-defined messaging.
    use type_c_interface::port::event::VdmData;

    use super::LocalPortId;

    /// Output from processing a vendor-defined message.
    #[derive(Copy, Clone, Debug)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct Output {
        /// The port that the VDM message is associated with.
        pub port: LocalPortId,
        /// VDM data
        pub vdm_data: VdmData,
    }
}

/// DP status changed output data
#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct OutputDpStatusChanged {
    /// Port ID
    pub port: LocalPortId,
    /// Port status
    pub status: DpStatus,
}

/// [`crate::wrapper::ControllerWrapper`] output
pub enum Output {
    /// No-op when nothing specific is needed
    Nop,
    /// Port status changed
    PortStatusChanged(OutputPortStatusChanged),
    /// PD alert
    PdAlert(OutputPdAlert),
    /// Vendor-defined messaging.
    Vdm(vdm::Output),
    /// Dp status update
    DpStatusUpdate(OutputDpStatusChanged),
}
