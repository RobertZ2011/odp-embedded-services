//! This module contains the `Controller` trait. Any types that implement this trait can be used with the `ControllerWrapper` struct
//! which provides a bridge between various service messages and the actual controller functions.
use core::array::from_fn;
use core::future::{pending, Future};

use embassy_futures::select::{select5, select_array, Either5};
use embassy_sync::mutex::Mutex;
use embassy_time::Instant;
use embedded_cfu_protocol::protocol_definitions::{FwUpdateOffer, FwUpdateOfferResponse, FwVersion};
use embedded_services::cfu::component::CfuDevice;
use embedded_services::ipc::deferred;
use embedded_services::power::policy::device::StateKind;
use embedded_services::power::policy::{self, action};
use embedded_services::transformers::object::{Object, RefGuard, RefMutGuard};
use embedded_services::type_c::controller::{self, Controller, PortStatus};
use embedded_services::type_c::event::{PortEvent, PortNotificationSingle, PortPending, PortStatusChanged};
use embedded_services::GlobalRawMutex;
use embedded_services::SyncCell;
use embedded_services::{debug, error, info, trace, warn};
use embedded_usb_pd::ado::Ado;
use embedded_usb_pd::{Error, PdError, PortId as LocalPortId};

use crate::wrapper::backing::Backing;
use crate::{PortEventStreamer, PortEventVariant};

pub mod backing;
mod cfu;
mod pd;
mod power;

/// Base interval for checking for FW update timeouts and recovery attempts
pub const DEFAULT_FW_UPDATE_TICK_INTERVAL_MS: u64 = 5000;
/// Default number of ticks before we consider a firmware update to have timed out
/// 300 seconds at 5 seconds per tick
pub const DEFAULT_FW_UPDATE_TIMEOUT_TICKS: u8 = 60;

/// Internal wrapper state
#[derive(Clone)]
pub struct InternalState<const N: usize> {
    /// If we're currently doing a firmware update
    pub fw_update_state: cfu::FwUpdateState,
    /// State used to keep track of where we are as we turn the event bitfields into a stream of events
    port_event_streaming_state: Option<PortEventStreamer>,
    /// Sink ready timeout values
    sink_ready_deadline: [Option<Instant>; N],
}

impl<const N: usize> Default for InternalState<N> {
    fn default() -> Self {
        Self {
            fw_update_state: cfu::FwUpdateState::Idle,
            port_event_streaming_state: None,
            sink_ready_deadline: [None; N],
        }
    }
}

/// Trait for validating firmware versions before applying an update
// TODO: remove this once we have a better framework for OEM customization
// See https://github.com/OpenDevicePartnership/embedded-services/issues/326
pub trait FwOfferValidator {
    /// Determine if we are accepting the firmware update offer, returns a CFU offer response
    fn validate(&self, current: FwVersion, offer: &FwUpdateOffer) -> FwUpdateOfferResponse;
}

pub enum Event<'a> {
    /// Port status changed
    PortStatusChanged(LocalPortId, PortStatusChanged),
    /// Port notification
    PortNotification(LocalPortId, PortNotificationSingle),
    /// Power policy command received
    PowerPolicyCommand(
        LocalPortId,
        deferred::Request<'a, GlobalRawMutex, policy::device::CommandData, policy::device::InternalResponseData>,
    ),
    /// Command from TCPM
    ControllerCommand(deferred::Request<'a, GlobalRawMutex, controller::Command, controller::Response<'static>>),
    /// Cfu event
    CfuEvent(cfu::Event),
}

/// Wrapper events
pub enum Output<'a> {
    /// No-op when nothing specific is needed
    Nop,
    /// Port status changed
    PortStatusChanged(LocalPortId, PortStatusChanged, PortStatus),
    /// PD alert
    PdAlert(LocalPortId, Ado),
    /// Power policy command received
    PowerPolicyCommand(
        LocalPortId,
        deferred::Request<'a, GlobalRawMutex, policy::device::CommandData, policy::device::InternalResponseData>,
        policy::device::InternalResponseData,
    ),
    /// TPCM command response
    ControllerCommand(
        deferred::Request<'a, GlobalRawMutex, controller::Command, controller::Response<'static>>,
        controller::Response<'static>,
    ),
    /// CFU response
    CfuResponse(embedded_services::cfu::component::InternalResponseData),
}

/// Takes an implementation of the `Controller` trait and wraps it with logic to handle
/// message passing and power-policy integration.
pub struct ControllerWrapper<'a, const N: usize, C: Controller, BACK: Backing<'a>, V: FwOfferValidator> {
    /// PD controller to interface with PD service
    pd_controller: controller::Device<'a>,
    /// Power policy devices to interface with power policy service
    power: [policy::device::Device; N],
    /// CFU device to interface with firmware update service
    cfu_device: CfuDevice,
    /// Internal state for the wrapper
    state: Mutex<GlobalRawMutex, InternalState<N>>,
    controller: Mutex<GlobalRawMutex, C>,
    active_events: [SyncCell<PortEvent>; N],
    /// Trait object for validating firmware versions
    fw_version_validator: V,
    /// FW update ticker used to check for timeouts and recovery attempts
    fw_update_ticker: Mutex<GlobalRawMutex, embassy_time::Ticker>,
    /// Channels and buffers used by the wrapper
    backing: Mutex<GlobalRawMutex, BACK>,
}

impl<'a, const N: usize, C: Controller, BACK: Backing<'a>, V: FwOfferValidator> ControllerWrapper<'a, N, C, BACK, V> {
    /// Create a new controller wrapper
    pub fn new(
        pd_controller: controller::Device<'a>,
        power: [policy::device::Device; N],
        cfu_device: CfuDevice,
        backing: BACK,
        controller: C,
        fw_version_validator: V,
    ) -> Self {
        Self {
            pd_controller,
            power,
            cfu_device,
            state: Mutex::new(Default::default()),
            controller: Mutex::new(controller),
            active_events: [const { SyncCell::new(PortEvent::none()) }; N],
            fw_version_validator,
            fw_update_ticker: Mutex::new(embassy_time::Ticker::every(embassy_time::Duration::from_millis(
                DEFAULT_FW_UPDATE_TICK_INTERVAL_MS,
            ))),
            backing: Mutex::new(backing),
        }
    }

    /// Get the power policy devices for this controller.
    pub fn power_policy_devices(&self) -> &[policy::device::Device] {
        &self.power
    }

    /// Handle a plug event
    async fn process_plug_event(
        &self,
        _controller: &mut C,
        power: &policy::device::Device,
        port: LocalPortId,
        status: &PortStatus,
    ) -> Result<(), Error<<C as Controller>::BusError>> {
        if port.0 > N as u8 {
            error!("Invalid port {}", port.0);
            return PdError::InvalidPort.into();
        }

        info!("Plug event");
        if status.is_connected() {
            info!("Plug inserted");

            // Recover if we're not in the correct state
            if power.state().await.kind() != StateKind::Detached {
                warn!("Power device not in detached state, recovering");
                if let Err(e) = power.detach().await {
                    error!("Error detaching power device: {:?}", e);
                    return PdError::Failed.into();
                }
            }

            if let Ok(state) = power.try_device_action::<action::Detached>().await {
                if let Err(e) = state.attach().await {
                    error!("Error attaching power device: {:?}", e);
                    return PdError::Failed.into();
                }
            } else {
                // This should never happen
                error!("Power device not in detached state");
                return PdError::InvalidMode.into();
            }
        } else {
            info!("Plug removed");
            if let Err(e) = power.detach().await {
                error!("Error detaching power device: {:?}", e);
                return PdError::Failed.into();
            };
        }

        Ok(())
    }

    /// Process port status changed events
    async fn process_port_status_changed<'b>(
        &self,
        controller: &mut C,
        state: &mut InternalState<N>,
        local_port_id: LocalPortId,
        status_event: PortStatusChanged,
    ) -> Result<Output<'b>, Error<<C as Controller>::BusError>> {
        let global_port_id = self
            .pd_controller
            .lookup_global_port(local_port_id)
            .map_err(Error::Pd)?;

        let status = controller.get_port_status(local_port_id, true).await?;
        trace!("Port{} status: {:#?}", global_port_id.0, status);

        let power = self.get_power_device(local_port_id).map_err(Error::Pd)?;
        trace!("Port{} status events: {:#?}", global_port_id.0, status_event);
        if status_event.plug_inserted_or_removed() {
            self.process_plug_event(controller, power, local_port_id, &status)
                .await?;
        }

        // Only notify power policy of a contract after Sink Ready event (always after explicit or implicit contract)
        if status_event.sink_ready() {
            self.process_new_consumer_contract(power, &status).await?;
        }

        if status_event.new_power_contract_as_provider() {
            self.process_new_provider_contract(power, &status).await?;
        }

        self.check_sink_ready_timeout(
            state,
            &status,
            local_port_id,
            status_event.new_power_contract_as_consumer(),
            status_event.sink_ready(),
        )
        .await?;

        Ok(Output::PortStatusChanged(local_port_id, status_event, status))
    }

    /// Finalize a port status change output
    async fn finalize_port_status_change(
        &self,
        local_port: LocalPortId,
        status_event: PortStatusChanged,
    ) -> Result<(), Error<<C as Controller>::BusError>> {
        if local_port.0 as usize >= N {
            error!("Invalid port {}", local_port.0);
            return Err(PdError::InvalidPort.into());
        }

        let global_port_id = self.pd_controller.lookup_global_port(local_port).map_err(Error::Pd)?;

        let mut events = self.active_events[local_port.0 as usize].get();
        events.status = events.status.union(status_event);
        self.active_events[local_port.0 as usize].set(events);

        let mut pending = PortPending::none();
        pending.pend_port(global_port_id.0 as usize);
        self.pd_controller.notify_ports(pending).await;

        Ok(())
    }

    /// Finalize a PD alert output
    async fn finalize_pd_alert(&self, port: LocalPortId, alert: Ado) -> Result<(), Error<<C as Controller>::BusError>> {
        let port_index = port.0 as usize;
        if port_index >= N {
            error!("Invalid port {}", port_index);
            return Err(PdError::InvalidPort.into());
        }

        // Buffer the alert
        let backing = self.backing.lock().await;
        let channel = backing.pd_alert_channel(port_index).await.ok_or(PdError::InvalidPort)?;
        channel.0.publish_immediate(alert);

        // Pend the alert
        let mut event = self.active_events[port_index].get();
        event.notification.set_alert(true);
        self.active_events[port_index].set(event);

        // Pend this port
        let mut pending = PortPending::none();
        pending.pend_port(port.0 as usize);
        self.pd_controller.notify_ports(pending).await;
        Ok(())
    }

    /// Wait for a pending port event
    async fn wait_port_pending(&self) -> Result<Event<'_>, Error<<C as Controller>::BusError>> {
        if self.state.lock().await.fw_update_state.in_progress() {
            // Don't process events while firmware update is in progress
            debug!("Firmware update in progress, ignoring port events");
            return pending().await;
        }

        // This loop is to ensure that if we finish streaming events we go back to waiting for the next port event
        loop {
            let streaming_state = self.state.lock().await.port_event_streaming_state;
            let mut stream = if let Some(streamer) = streaming_state {
                // If we're converting the bitfields into an event stream yield first to prevent starving other tasks
                embassy_futures::yield_now().await;
                streamer
            } else {
                // Otherwise, wait for the next port event
                self.controller.lock().await.wait_port_event().await?;
                let pending: PortPending = FromIterator::from_iter(0..N);
                PortEventStreamer::new(pending.into_iter())
            };

            if let Some((port_id, event)) = stream
                .next(async |port_id| {
                    self.controller
                        .lock()
                        .await
                        .clear_port_events(LocalPortId(port_id as u8))
                        .await
                })
                .await?
            {
                let port_id = LocalPortId(port_id as u8);
                self.state.lock().await.port_event_streaming_state = Some(stream);
                match event {
                    PortEventVariant::StatusChanged(status_event) => {
                        return Ok(Event::PortStatusChanged(port_id, status_event));
                    }
                    PortEventVariant::Notification(notification) => {
                        return Ok(Event::PortNotification(port_id, notification));
                    }
                }
            } else {
                self.state.lock().await.port_event_streaming_state = None;
            }
        }
    }

    /// Wait for the next event
    pub async fn wait_next(&self) -> Result<Event<'_>, Error<<C as Controller>::BusError>> {
        let event = select5(
            self.wait_port_pending(),
            self.wait_power_command(),
            self.pd_controller.receive(),
            self.wait_cfu_command(),
            self.wait_sink_ready_timeout(),
        )
        .await;
        match event {
            Either5::First(event) => event,
            Either5::Second((port, request)) => Ok(Event::PowerPolicyCommand(port, request)),
            Either5::Third(request) => Ok(Event::ControllerCommand(request)),
            Either5::Fourth(event) => Ok(Event::CfuEvent(event)),
            Either5::Fifth(port) => {
                // Sink ready timeout event
                debug!("Port{0}: Sink ready timeout", port.0);
                self.state.lock().await.sink_ready_deadline[port.0 as usize] = None;
                let mut event = PortStatusChanged::none();
                event.set_sink_ready(true);
                Ok(Event::PortStatusChanged(port, event))
            }
        }
    }

    /// Process a port notification
    async fn process_port_notification<'b>(
        &self,
        controller: &mut C,
        port: LocalPortId,
        notification: PortNotificationSingle,
    ) -> Result<Output<'b>, Error<<C as Controller>::BusError>> {
        match notification {
            PortNotificationSingle::Alert => {
                let ado = controller.get_pd_alert(port).await?;
                trace!("Port{}: PD alert: {:#?}", port.0, ado);
                if let Some(ado) = ado {
                    Ok(Output::PdAlert(port, ado))
                } else {
                    // For some reason we didn't read an alert, nothing to do
                    Ok(Output::Nop)
                }
            }
            rest => {
                trace!("Port{}: Notification: {:#?}", port.0, rest);
                Ok(Output::Nop)
            }
        }
    }

    /// Top-level processing function
    /// Only call this fn from one place in a loop. Otherwise a deadlock could occur.
    pub async fn process_event<'b>(&self, event: Event<'b>) -> Result<Output<'b>, Error<<C as Controller>::BusError>> {
        let mut controller = self.controller.lock().await;
        let mut state = self.state.lock().await;
        match event {
            Event::PortStatusChanged(port_id, status_event) => {
                self.process_port_status_changed(&mut controller, &mut state, port_id, status_event)
                    .await
            }
            Event::PowerPolicyCommand(port, request) => {
                let response = self
                    .process_power_command(&mut controller, &mut state, port, &request.command)
                    .await;
                Ok(Output::PowerPolicyCommand(port, request, response))
            }
            Event::ControllerCommand(request) => {
                let response = self
                    .process_pd_command(&mut controller, &mut state, &request.command)
                    .await;
                Ok(Output::ControllerCommand(request, response))
            }
            Event::CfuEvent(event) => match event {
                cfu::Event::Request(request) => {
                    let response = self.process_cfu_command(&mut controller, &mut state, &request).await;
                    Ok(Output::CfuResponse(response))
                }
                cfu::Event::RecoveryTick => {
                    // FW Update tick, process timeouts and recovery attempts
                    self.process_cfu_tick(&mut controller, &mut state).await;
                    Ok(Output::Nop)
                }
            },
            Event::PortNotification(port, notification) => {
                self.process_port_notification(&mut controller, port, notification)
                    .await
            }
        }
    }

    /// Event loop finalize
    pub async fn finalize<'b>(&self, output: Output<'b>) -> Result<(), Error<<C as Controller>::BusError>> {
        match output {
            Output::Nop => Ok(()),
            Output::PortStatusChanged(port, status_event, _) => {
                self.finalize_port_status_change(port, status_event).await
            }
            Output::PdAlert(port, alert) => self.finalize_pd_alert(port, alert).await,
            Output::PowerPolicyCommand(_port, request, response) => {
                request.respond(response);
                Ok(())
            }
            Output::ControllerCommand(request, response) => {
                request.respond(response);
                Ok(())
            }
            Output::CfuResponse(response) => {
                self.send_cfu_response(response).await;
                Ok(())
            }
        }
    }

    /// Combined processing function
    pub async fn process_next_event(&self) -> Result<(), Error<<C as Controller>::BusError>> {
        let event = self.wait_next().await?;
        let output = self.process_event(event).await?;
        self.finalize(output).await
    }

    /// Register all devices with their respective services
    pub async fn register(&'static self) -> Result<(), Error<<C as Controller>::BusError>> {
        for device in &self.power {
            policy::register_device(device).await.map_err(|_| {
                error!(
                    "Controller{}: Failed to register power device {}",
                    self.pd_controller.id().0,
                    device.id().0
                );
                Error::Pd(PdError::Failed)
            })?;
        }

        controller::register_controller(&self.pd_controller)
            .await
            .map_err(|_| {
                error!(
                    "Controller{}: Failed to register PD controller",
                    self.pd_controller.id().0
                );
                Error::Pd(PdError::Failed)
            })?;

        //TODO: Remove when we have a more general framework in place
        embedded_services::cfu::register_device(&self.cfu_device)
            .await
            .map_err(|_| {
                error!("Controller{}: Failed to register CFU device", self.pd_controller.id().0);
                Error::Pd(PdError::Failed)
            })?;
        Ok(())
    }
}

impl<'a, const N: usize, C: Controller, BACK: Backing<'a>, V: FwOfferValidator> Object<C>
    for ControllerWrapper<'a, N, C, BACK, V>
{
    fn get_inner(&self) -> impl Future<Output = impl RefGuard<C>> {
        self.controller.lock()
    }

    fn get_inner_mut(&self) -> impl Future<Output = impl RefMutGuard<C>> {
        self.controller.lock()
    }
}
