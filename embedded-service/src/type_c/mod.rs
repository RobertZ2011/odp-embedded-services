//! Type-C service

pub mod controller;
pub mod ucsi;

/// Global port ID, used to unique identify a port
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GlobalPortId(pub u8);

/// Controller ID
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ControllerId(pub u8);
