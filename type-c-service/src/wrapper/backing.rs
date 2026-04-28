//! Various types of state and objects required for [`crate::wrapper::ControllerWrapper`].
//!
//! TODO: update this documentation when the type-C service is refactored
//!
use embassy_sync::{blocking_mutex::raw::RawMutex, mutex::Mutex};

use embedded_services::{event, sync::Lockable};

use embedded_usb_pd::LocalPortId;
use type_c_interface::port::{Controller, ControllerId, PortRegistration, PortStatus, event::PortStatusEventBitfield};

use crate::wrapper::proxy::PowerProxyDevice;

/// Service registration objects
pub struct Registration<'a> {
    pub context: &'a type_c_interface::service::context::Context,
    pub pd_controller: &'a type_c_interface::port::Device<'a>,
}

/// Base storage
pub struct Storage<'a, const N: usize> {
    // Registration-related
    context: &'a type_c_interface::service::context::Context,
    controller_id: ControllerId,
    pd_ports: [PortRegistration; N],
}

impl<'a, const N: usize> Storage<'a, N> {
    pub fn new(
        context: &'a type_c_interface::service::context::Context,
        controller_id: ControllerId,
        pd_ports: [PortRegistration; N],
    ) -> Self {
        Self {
            context,
            controller_id,
            pd_ports,
        }
    }

    /// Create intermediate storage from this storage
    pub fn try_create_intermediate<
        M: RawMutex,
        C: Lockable<Inner: Controller>,
        S: event::Sender<power_policy_interface::psu::event::EventData>,
    >(
        &'a self,
        power_policy_init: [(&'static str, LocalPortId, &'a C, S); N],
    ) -> Option<IntermediateStorage<'a, N, M, C, S>> {
        IntermediateStorage::try_from_storage(self, power_policy_init)
    }
}

pub struct Port<
    'a,
    M: RawMutex,
    C: Lockable<Inner: Controller>,
    S: event::Sender<power_policy_interface::psu::event::EventData>,
> {
    pub proxy: Mutex<M, PowerProxyDevice<'a, C>>,
    pub state: Mutex<M, PortState<S>>,
}

pub struct PortState<S: event::Sender<power_policy_interface::psu::event::EventData>> {
    /// Cached port status
    pub(crate) status: PortStatus,
    /// Software status event
    pub(crate) sw_status_event: PortStatusEventBitfield,
    /// Sender to send events to the power policy service
    pub(crate) power_policy_sender: S,
}

impl<S: event::Sender<power_policy_interface::psu::event::EventData>> PortState<S> {
    pub fn new(power_policy_sender: S) -> Self {
        Self {
            status: PortStatus::default(),
            sw_status_event: PortStatusEventBitfield::default(),
            power_policy_sender,
        }
    }
}

/// Intermediate storage that holds power proxy devices
pub struct IntermediateStorage<
    'a,
    const N: usize,
    M: RawMutex,
    C: Lockable<Inner: Controller>,
    S: event::Sender<power_policy_interface::psu::event::EventData>,
> {
    storage: &'a Storage<'a, N>,
    ports: [Port<'a, M, C, S>; N],
}

impl<
    'a,
    const N: usize,
    M: RawMutex,
    C: Lockable<Inner: Controller>,
    S: event::Sender<power_policy_interface::psu::event::EventData>,
> IntermediateStorage<'a, N, M, C, S>
{
    fn try_from_storage(
        storage: &'a Storage<'a, N>,
        power_policy_init: [(&'static str, LocalPortId, &'a C, S); N],
    ) -> Option<IntermediateStorage<'a, N, M, C, S>> {
        let mut ports = heapless::Vec::<_, N>::new();

        for (name, port_id, controller, policy_sender) in power_policy_init.into_iter() {
            ports
                .push(Port {
                    proxy: Mutex::new(PowerProxyDevice::new(name, port_id, controller)),
                    state: Mutex::new(PortState::new(policy_sender)),
                })
                .ok()?;
        }

        Some(Self {
            storage,
            ports: ports.into_array().ok()?,
        })
    }

    /// Create referenced storage from this intermediate storage
    pub fn try_create_referenced<'b>(&'b self) -> Option<ReferencedStorage<'b, N, M, C, S>>
    where
        'b: 'a,
    {
        ReferencedStorage::try_from_intermediate(self)
    }
}

/// Contains any values that need to reference [`Storage`]
///
/// To simplify usage, we use interior mutability through a ref cell to avoid having to declare the state
/// completely separately.
pub struct ReferencedStorage<
    'a,
    const N: usize,
    M: RawMutex,
    C: Lockable<Inner: Controller>,
    S: event::Sender<power_policy_interface::psu::event::EventData>,
> {
    intermediate: &'a IntermediateStorage<'a, N, M, C, S>,
    pub pd_controller: type_c_interface::port::Device<'a>,
}

impl<
    'a,
    const N: usize,
    M: RawMutex,
    C: Lockable<Inner: Controller>,
    S: event::Sender<power_policy_interface::psu::event::EventData>,
> ReferencedStorage<'a, N, M, C, S>
{
    /// Create a new referenced storage from the given intermediate storage
    fn try_from_intermediate(intermediate: &'a IntermediateStorage<'a, N, M, C, S>) -> Option<Self> {
        Some(Self {
            intermediate,
            pd_controller: type_c_interface::port::Device::new(
                intermediate.storage.controller_id,
                intermediate.storage.pd_ports.as_slice(),
            ),
        })
    }

    /// Creates the backing, returns `None` if a backing has already been created
    pub fn create_backing<'b>(&'b self) -> Backing<'b, M, C, S>
    where
        'b: 'a,
    {
        Backing {
            registration: Registration {
                context: self.intermediate.storage.context,
                pd_controller: &self.pd_controller,
            },
            ports: &self.intermediate.ports,
        }
    }
}

/// Wrapper around registration and type-erased state
pub struct Backing<
    'a,
    M: RawMutex,
    C: Lockable<Inner: Controller>,
    S: event::Sender<power_policy_interface::psu::event::EventData>,
> {
    pub(crate) registration: Registration<'a>,
    pub(crate) ports: &'a [Port<'a, M, C, S>],
}
