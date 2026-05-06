use core::marker::PhantomData;

use embedded_services::event::Sender as _;
use embedded_services::{debug, error, info, trace};
use embedded_usb_pd::GlobalPortId;
use embedded_usb_pd::PdError as Error;
use power_policy_interface::service::event::EventData as PowerPolicyEventData;
use type_c_interface::control::pd::PortStatus;
use type_c_interface::service::event::{DebugAccessory, PortEvent, PortEventData};

use type_c_interface::port::event::PortStatusEventBitfield;
use type_c_interface::service::event::Event as ServiceEvent;

use crate::service::registration::Registration;

pub mod config;
pub mod event_receiver;
mod power;
pub mod registration;
mod ucsi;

const MAX_SUPPORTED_PORTS: usize = 4;

/// Type-C service state
#[derive(Default)]
struct State {
    /// Current port status
    port_status: [PortStatus; MAX_SUPPORTED_PORTS],
    /// UCSI state
    ucsi: ucsi::State,
}

/// Type-C service
///
/// Constructing a Service is the first step in using the Type-C service.
/// Arguments should be an initialized context
pub struct Service<'device, Reg: Registration<'device>> {
    /// Current state
    state: State,
    /// Config
    config: config::Config,
    /// Service registration
    registration: Reg,
    _phantom: PhantomData<&'device ()>,
}

/// Type-C service events
#[derive(Clone)]
pub enum Event {
    /// Port event
    PortEvent(PortEvent),
    /// Power policy event
    PowerPolicy(PowerPolicyEventData),
}

impl<'a, Reg: Registration<'a>> Service<'a, Reg> {
    /// Create a new service the given configuration
    pub fn create(config: config::Config, registration: Reg) -> Self {
        Self {
            state: State::default(),
            config,
            registration,
            _phantom: PhantomData,
        }
    }

    /// Get the cached port status
    pub fn get_cached_port_status(&self, port_id: GlobalPortId) -> Result<PortStatus, Error> {
        Ok(*self
            .state
            .port_status
            .get(port_id.0 as usize)
            .ok_or(Error::InvalidPort)?)
    }

    /// Set the cached port status
    fn set_cached_port_status(&mut self, port_id: GlobalPortId, status: PortStatus) -> Result<(), Error> {
        *self
            .state
            .port_status
            .get_mut(port_id.0 as usize)
            .ok_or(Error::InvalidPort)? = status;
        Ok(())
    }

    /// Look up the port for a given global port ID
    fn lookup_port(&self, port_id: GlobalPortId) -> Result<&Reg::Port, Error> {
        self.registration
            .ports()
            .get(port_id.0 as usize)
            .ok_or(Error::InvalidPort)
            .map(|port| *port)
    }

    /// Send an event to all registered listeners
    async fn broadcast_event(&mut self, event: ServiceEvent) {
        for sender in self.registration.event_senders() {
            sender.send(event).await;
        }
    }

    /// Process events for a specific port
    async fn process_port_status_event(
        &mut self,
        port_id: GlobalPortId,
        event: PortStatusEventBitfield,
        status: PortStatus,
    ) -> Result<(), Error> {
        let old_status = self.get_cached_port_status(port_id)?;

        debug!("Port{}: Event: {:#?}", port_id.0, event);
        debug!("Port{} Previous status: {:#?}", port_id.0, old_status);
        debug!("Port{} Status: {:#?}", port_id.0, status);

        let connection_changed = status.is_connected() != old_status.is_connected();
        if connection_changed && (status.is_debug_accessory() || old_status.is_debug_accessory()) {
            // Notify that a debug connection has connected/disconnected
            if status.is_connected() {
                debug!("Port{}: Debug accessory connected", port_id.0);
            } else {
                debug!("Port{}: Debug accessory disconnected", port_id.0);
            }

            self.broadcast_event(ServiceEvent::DebugAccessory(DebugAccessory {
                port: port_id,
                connected: status.is_connected(),
            }))
            .await;
        }

        self.set_cached_port_status(port_id, status)?;
        self.handle_ucsi_port_event(port_id, event, &status).await;

        Ok(())
    }

    async fn process_port_event(&mut self, event: &PortEvent) -> Result<(), Error> {
        match &event.event {
            PortEventData::StatusChanged(status_event) => {
                self.process_port_status_event(event.port, status_event.status_event, status_event.current_status)
                    .await
            }
            unhandled => {
                // Currently just log notifications, but may want to do more in the future
                debug!("Port{}: Received notification event: {:#?}", event.port.0, unhandled);
                Ok(())
            }
        }
    }

    /// Process the given event
    pub async fn process_event(&mut self, event: Event) -> Result<(), Error> {
        match event {
            Event::PortEvent(event) => {
                trace!("Port{}: Processing port event", event.port.0);
                self.process_port_event(&event).await
            }
            Event::PowerPolicy(event) => {
                trace!("Processing power policy event");
                self.process_power_policy_event(&event).await
            }
        }
    }
}
