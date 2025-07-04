use crate::device::Device;
use crate::device::{self, DeviceId};
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::TrySendError};
use embassy_time::{with_timeout, Duration};
use embedded_services::{debug, error, info, intrusive_list, trace, warn, IntrusiveList};

use core::cell::Cell;
use core::ops::DerefMut;

/// Battery service states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum State {
    NotPresent,

    Present(PresentSubstate),
}

/// Present state substates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PresentSubstate {
    NotOperational,
    Operational(OperationalSubstate),
}

/// Operational state substates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum OperationalSubstate {
    Init,
    Polling,
}

/// Battery state machine events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum BatteryEventInner {
    DoInit,
    PollDynamicData,
    PollStaticData,
    Timeout,
    Oem(u8, &'static [u8]),
}

/// Battery state machine response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum InnerStateMachineResponse {
    Complete,
}

/// Battery state machine errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum StateMachineError {
    DeviceTimeout,
    DeviceError,
    InvalidActionInState,
}

/// External battery state machine response.  
type StateMachineResponse = Result<InnerStateMachineResponse, StateMachineError>;

/// Battery service context response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ContextResponse {
    Ack,
}

/// Battery service context error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ContextError {
    DeviceNotFound,
    Timeout,
    StateError(StateMachineError),
}

/// External battery service context response.
pub type BatteryResponse = Result<ContextResponse, ContextError>;

/// External battery state machine event wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct BatteryEvent {
    pub event: BatteryEventInner,
    pub device_id: DeviceId,
}

/// Battery service context, hardware agnostic state.
pub struct Context {
    fuel_gauges: IntrusiveList,
    state: Mutex<NoopRawMutex, State>,
    battery_event: Channel<NoopRawMutex, BatteryEvent, 1>,
    battery_response: Channel<NoopRawMutex, BatteryResponse, 1>,
    no_op_retry_count: Cell<usize>,
    config: Config,
}

pub struct Config {
    state_machine_timeout_ms: Duration,
    no_op_max_retries: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            state_machine_timeout_ms: Duration::from_secs(120),
            no_op_max_retries: 5,
        }
    }
}

impl Context {
    /// Create a new context instance.
    pub fn new() -> Self {
        Self {
            fuel_gauges: IntrusiveList::new(),
            state: Mutex::new(State::NotPresent),
            battery_event: Channel::new(),
            battery_response: Channel::new(),
            no_op_retry_count: Cell::new(0),
            config: Default::default(),
        }
    }

    pub fn new_with_config(config: Config) -> Self {
        Self {
            fuel_gauges: IntrusiveList::new(),
            state: Mutex::new(State::NotPresent),
            battery_event: Channel::new(),
            battery_response: Channel::new(),
            no_op_retry_count: Cell::new(0),
            config,
        }
    }

    /// Get global state machine timeout.
    fn get_state_machine_timeout(&self) -> Duration {
        self.config.state_machine_timeout_ms
    }

    /// Get global state machine NotOperational max # of retries.
    fn get_state_machine_max_retries(&self) -> usize {
        self.config.no_op_max_retries
    }

    /// Get global state machine NotOperational retry count.
    fn get_state_machine_retry_count(&self) -> usize {
        self.no_op_retry_count.get()
    }

    /// Set global state machine NotOperational retry count.
    fn set_state_machine_retry_count(&self, retry_count: usize) {
        self.no_op_retry_count.set(retry_count)
    }

    /// Main processing function.
    pub async fn process(&self, event: BatteryEvent) {
        let res = with_timeout(self.get_state_machine_timeout(), self.do_state_machine(event)).await;
        match res {
            Ok(sm_res) => match sm_res {
                Ok(_) => {
                    debug!("Battery state machine completed for event {:?}", event);
                    self.battery_response.send(Ok(ContextResponse::Ack)).await;
                }
                Err(e) => {
                    error!("Battery state machine completed but errored {:?}", event);
                    self.battery_response.send(Err(ContextError::StateError(e))).await;
                }
            },
            Err(_) => {
                error!("Battery state machine timeout!");
                // Should be infalliable
                self.do_state_machine(BatteryEvent {
                    event: BatteryEventInner::Timeout,
                    device_id: event.device_id,
                })
                .await
                .expect("Error type is Infallible");
                self.battery_response.send(Err(ContextError::Timeout)).await;
            }
        };
    }

    /// Process and validate event before running state machine.
    fn handle_event(&self, state: &mut State, event: BatteryEventInner) -> Result<State, StateMachineError> {
        match event {
            BatteryEventInner::DoInit => {
                if *state != State::NotPresent {
                    warn!(
                        "Battery Service: received init command when not in init state. State machine reinitializing"
                    );
                    trace!("State = {:?}", *state);
                }
                Ok(State::NotPresent)
            }
            BatteryEventInner::PollDynamicData => {
                if *state != State::Present(PresentSubstate::Operational(OperationalSubstate::Polling)) {
                    error!("Battery Service: received dynamic poll request while not in polling state");
                    trace!("State = {:?}", *state);
                    Err(StateMachineError::InvalidActionInState)
                } else {
                    Ok(State::Present(PresentSubstate::Operational(
                        OperationalSubstate::Polling,
                    )))
                }
            }
            BatteryEventInner::PollStaticData => {
                if *state != State::Present(PresentSubstate::Operational(OperationalSubstate::Polling)) {
                    error!("Battery Service: received static poll request while not in polling state");
                    trace!("State = {:?}", *state);
                    Err(StateMachineError::InvalidActionInState)
                } else {
                    Ok(State::Present(PresentSubstate::Operational(OperationalSubstate::Init)))
                }
            }
            BatteryEventInner::Timeout => {
                warn!("Battery Service: received timeout command");
                trace!("State = {:?}", *state);
                Ok(State::Present(PresentSubstate::NotOperational))
            }
            BatteryEventInner::Oem(_, _items) => todo!(),
        }
    }

    /// Main battery service state machine
    async fn do_state_machine(&self, event: BatteryEvent) -> StateMachineResponse {
        let mut state = self.state.lock().await;

        // BatteryEventInner can transition state, or an invalid event can cause the state machine to return
        match self.handle_event(state.deref_mut(), event.event) {
            Ok(new_state) => *state = new_state,
            Err(err) => return Err(err),
        }

        match *state {
            State::NotPresent => {
                info!("Initializing fuel gauge with ID {:?}", event.device_id);
                if self
                    .execute_device_command(event.device_id, device::Command::Ping)
                    .await
                    .is_err()
                {
                    error!("Error pinging fuel gauge with ID {:?}", event.device_id);
                    return Err(StateMachineError::DeviceError);
                }
                if self
                    .execute_device_command(event.device_id, device::Command::Initialize)
                    .await
                    .is_err()
                {
                    error!("Error initializing fuel gauge with ID {:?}", event.device_id);
                    return Err(StateMachineError::DeviceError);
                }

                *state = State::Present(PresentSubstate::Operational(OperationalSubstate::Init));
                Ok(InnerStateMachineResponse::Complete)
            }
            State::Present(substate) => match substate {
                PresentSubstate::NotOperational => {
                    self.set_state_machine_retry_count(self.get_state_machine_max_retries() + 1);
                    match self
                        .execute_device_command(event.device_id, device::Command::Ping)
                        .await
                    {
                        Ok(Ok(device::InternalResponse::Complete)) => {
                            info!("Fuel gauge id: {:?} re-established communication!", event.device_id);
                            *state = State::Present(PresentSubstate::Operational(OperationalSubstate::Init));
                            self.set_state_machine_retry_count(0);
                            Ok(InnerStateMachineResponse::Complete)
                            // Do not continue execution.
                        }
                        Ok(Err(fg_err)) => {
                            error!(
                                "Fuel gauge {:?} failed to ping with error {:?}",
                                event.device_id, fg_err
                            );
                            // Do not continue execution, if we got to this point it's because we errored.
                            // Require re-executing manual CheckReady calls. If we go over the max retries,
                            // transition to the NotPresent state.
                            if self.get_state_machine_retry_count() > self.get_state_machine_max_retries() {
                                *state = State::NotPresent;
                            }
                            Err(StateMachineError::DeviceTimeout)
                        }
                        Err(ctx_err) => {
                            error!(
                                "Battery state machine NotOperational error: {:?} for ID {:?}",
                                ctx_err, event.device_id
                            );
                            // Do not continue execution, if we got to this point it's because we errored.
                            // Require re-executing manual CheckReady calls. If we go over the max retries,
                            // transition to the NotPresent state.
                            if self.get_state_machine_retry_count() > self.get_state_machine_max_retries() {
                                *state = State::NotPresent;
                            }
                            Err(StateMachineError::DeviceTimeout)
                        }
                    }
                }
                PresentSubstate::Operational(operational_substate) => match operational_substate {
                    OperationalSubstate::Init => {
                        // Collect static data
                        // TODO: Add retry logic
                        info!("Collecting fuel gauge static cache with ID {:?}", event.device_id);
                        if self
                            .execute_device_command(event.device_id, device::Command::UpdateStaticCache)
                            .await
                            .is_err()
                        {
                            error!("Error updating fuel gauge static cache with ID {:?}", event.device_id);
                            return Err(StateMachineError::DeviceError);
                        }
                        *state = State::Present(PresentSubstate::Operational(OperationalSubstate::Polling));
                        Ok(InnerStateMachineResponse::Complete)
                    }
                    OperationalSubstate::Polling => {
                        // Collect dynamic data
                        // TODO: Add retry logic
                        info!("Collecting fuel gauge dynamic cache with ID {:?}", event.device_id);
                        if self
                            .execute_device_command(event.device_id, device::Command::UpdateDynamicCache)
                            .await
                            .is_err()
                        {
                            error!(
                                "Error initializing fuel gauge dynamic cache with ID {:?}",
                                event.device_id
                            );
                            return Err(StateMachineError::DeviceError);
                        }
                        Ok(InnerStateMachineResponse::Complete)
                    }
                },
            },
        }
    }

    fn get_fuel_gauge(&self, id: DeviceId) -> Option<&'static Device> {
        for device in &self.fuel_gauges {
            if let Some(data) = device.data::<Device>() {
                if data.id() == id {
                    return Some(data);
                }
            } else {
                error!("Non-device located in devices list");
            }
        }
        None
    }

    /// Register fuel gauge device with the context instance.
    pub async fn register_fuel_gauge(&self, device: &'static Device) -> Result<(), intrusive_list::Error> {
        if self.get_fuel_gauge(device.id()).is_some() {
            return Err(embedded_services::Error::NodeAlreadyInList);
        }

        self.fuel_gauges.push(device)
    }

    async fn send_event(&self, event: BatteryEvent) {
        self.battery_event.send(event).await;
    }

    pub async fn wait_response(&self) -> BatteryResponse {
        self.battery_response.receive().await
    }

    /// Send an event to the context and wait for a response.
    pub async fn execute_event(&self, event: BatteryEvent) -> BatteryResponse {
        self.send_event(event).await;
        self.wait_response().await
    }

    pub fn send_event_no_wait(&self, event: BatteryEvent) -> Result<(), TrySendError<BatteryEvent>> {
        self.battery_event.try_send(event)
    }

    /// Wait for battery event.
    pub async fn wait_event(&self) -> BatteryEvent {
        self.battery_event.receive().await
    }

    pub async fn get_state(&self) -> State {
        *self.state.lock().await
    }

    async fn execute_device_command(
        &self,
        id: DeviceId,
        command: device::Command,
    ) -> Result<device::Response, ContextError> {
        // Get ID
        let device = match self.get_fuel_gauge(id) {
            Some(device) => device,
            None => {
                // TODO: Send error response
                error!("Fuel gauge with ID {:?} not found", id);
                return Err(ContextError::DeviceNotFound);
            }
        };

        match with_timeout(device.get_timeout(), device.execute_command(command)).await {
            Ok(res) => Ok(res),
            Err(_) => {
                error!("Device timed out when executing command {:?}", command);
                Err(ContextError::Timeout)
            }
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
