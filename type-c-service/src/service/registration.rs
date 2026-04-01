//! Types  and traits related to registration for the type-C service.

use embedded_services::sync::Lockable;
use type_c_interface::port::pd::Pd;

/// Trait for type-C service registration.
pub trait Registration<'device> {
    type Port: Lockable<Inner: Pd>;

    fn ports(&self) -> &[&'device Self::Port];
}

/// A registration implementation based around arrays
pub struct ArrayRegistration<'device, Port: Lockable<Inner: Pd> + 'device, const PORT_COUNT: usize> {
    /// Array of registered ports
    pub ports: [&'device Port; PORT_COUNT],
}

impl<'device, Port: Lockable<Inner: Pd> + 'device, const PORT_COUNT: usize> Registration<'device>
    for ArrayRegistration<'device, Port, PORT_COUNT>
{
    type Port = Port;

    fn ports(&self) -> &[&'device Self::Port] {
        &self.ports
    }
}
