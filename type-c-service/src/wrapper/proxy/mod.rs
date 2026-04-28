use embedded_services::{named::Named, sync::Lockable};
use embedded_usb_pd::LocalPortId;
use type_c_interface::port::Controller;

mod power;

pub struct PowerProxyDevice<'device, C: Lockable<Inner: Controller>> {
    /// Local port
    port: LocalPortId,
    /// Controller
    controller: &'device C,
    /// Per-port PSU state
    pub(crate) psu_state: power_policy_interface::psu::State,
    name: &'static str,
}

impl<'device, C: Lockable<Inner: Controller>> PowerProxyDevice<'device, C> {
    pub fn new(name: &'static str, port: LocalPortId, controller: &'device C) -> Self {
        Self {
            name,
            controller,
            port,
            psu_state: power_policy_interface::psu::State::default(),
        }
    }
}

impl<'device, C: Lockable<Inner: Controller>> Named for PowerProxyDevice<'device, C> {
    fn name(&self) -> &'static str {
        self.name
    }
}
