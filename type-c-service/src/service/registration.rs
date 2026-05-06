//! Code related to registration with the type-C service

use embedded_services::{event::Sender, sync::Lockable};
use embedded_usb_pd::{GlobalPortId, LocalPortId};
use type_c_interface::port::pd::Pd;
use type_c_interface::service::event::Event as ServiceEvent;
use type_c_interface::ucsi::Lpm as UcsiLpm;

/// Registration trait that abstracts over various registration details.
pub trait Registration<'device> {
    type Port: Lockable<Inner: Pd + UcsiLpm> + 'device;
    type ServiceSender: Sender<ServiceEvent>;

    /// Returns a slice to access ports
    fn ports(&self) -> &[&'device Self::Port];
    /// Returns a slice to access type-c event senders
    fn event_senders(&mut self) -> &mut [Self::ServiceSender];
    /// Returns the ucsi local port ID for a given global port
    fn ucsi_local_port_id(&self, global_port: GlobalPortId) -> Option<LocalPortId>;
}

pub struct PortData {
    /// local port ID
    pub local_port: Option<LocalPortId>,
}

/// A registration implementation based around arrays
pub struct ArrayRegistration<
    'device,
    Port: Lockable<Inner: Pd + UcsiLpm> + 'device,
    const PORT_COUNT: usize,
    ServiceSender: Sender<ServiceEvent>,
    const SERVICE_SENDER_COUNT: usize,
> {
    /// Array of registered ports
    pub ports: [&'device Port; PORT_COUNT],
    /// Array of local port data, indexed by global port ID
    pub port_data: [PortData; PORT_COUNT],
    /// Array of service event senders
    pub service_senders: [ServiceSender; SERVICE_SENDER_COUNT],
}

impl<
    'device,
    Port: Lockable<Inner: Pd + UcsiLpm> + 'device,
    const PORT_COUNT: usize,
    ServiceSender: Sender<ServiceEvent>,
    const SERVICE_SENDER_COUNT: usize,
> Registration<'device> for ArrayRegistration<'device, Port, PORT_COUNT, ServiceSender, SERVICE_SENDER_COUNT>
{
    type Port = Port;
    type ServiceSender = ServiceSender;

    fn event_senders(&mut self) -> &mut [Self::ServiceSender] {
        &mut self.service_senders
    }

    fn ports(&self) -> &[&'device Self::Port] {
        &self.ports
    }

    fn ucsi_local_port_id(&self, global_port: GlobalPortId) -> Option<LocalPortId> {
        self.port_data
            .get(global_port.0 as usize)
            .and_then(|data| data.local_port)
    }
}
