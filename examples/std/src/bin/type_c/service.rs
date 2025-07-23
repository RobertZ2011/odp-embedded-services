use embassy_executor::{Executor, Spawner};
use embassy_sync::once_lock::OnceLock;
use embassy_time::Timer;
use embedded_cfu_protocol::protocol_definitions::{FwUpdateOfferResponse, HostToken};
use embedded_services::comms;
use embedded_services::power::{self, policy};
use embedded_services::transformers::object::Object;
use embedded_services::type_c::{ControllerId, controller};
use embedded_usb_pd::Error;
use embedded_usb_pd::GlobalPortId;
use embedded_usb_pd::PortId as LocalPortId;
use embedded_usb_pd::ado::Ado;
use embedded_usb_pd::type_c::Current;
use log::*;
use static_cell::StaticCell;
use type_c_service::wrapper::Event;

const CONTROLLER0: ControllerId = ControllerId(0);
const PORT0: GlobalPortId = GlobalPortId(0);
const POWER0: power::policy::DeviceId = power::policy::DeviceId(0);
const DELAY_MS: u64 = 1000;

mod test_controller {
    use std::cell::Cell;

    use embassy_sync::{mutex::Mutex, signal::Signal};
    use embedded_services::{
        GlobalRawMutex,
        type_c::{
            controller::{Contract, ControllerStatus, PortStatus, RetimerFwUpdateState},
            event::PortEvent,
        },
    };
    use embedded_usb_pd::type_c::ConnectionState;

    use super::*;

    pub struct ControllerState {
        events: Signal<GlobalRawMutex, PortEvent>,
        status: Mutex<GlobalRawMutex, PortStatus>,
        pd_alert: Mutex<GlobalRawMutex, Option<Ado>>,
    }

    impl ControllerState {
        pub fn new() -> Self {
            Self {
                events: Signal::new(),
                status: Mutex::new(PortStatus::default()),
                pd_alert: Mutex::new(None),
            }
        }

        /// Simulate a connection
        pub async fn connect(&self, contract: Contract, debug: bool) {
            let mut status = PortStatus::new();
            status.connection_state = Some(if debug {
                ConnectionState::DebugAccessory
            } else {
                ConnectionState::Attached
            });
            match contract {
                Contract::Source(capability) => {
                    status.available_source_contract = Some(capability);
                }
                Contract::Sink(capability) => {
                    status.available_sink_contract = Some(capability);
                }
            }
            *self.status.lock().await = status;

            let mut events = PortEvent::none();
            events.status.set_plug_inserted_or_removed(true);
            events.status.set_new_power_contract_as_consumer(true);
            events.status.set_sink_ready(true);
            self.events.signal(events);
        }

        /// Simulate a sink connecting
        pub async fn connect_sink(&self, current: Current) {
            self.connect(Contract::Sink(current.into()), false).await;
        }

        /// Simulate a disconnection
        pub async fn disconnect(&self) {
            *self.status.lock().await = PortStatus::default();

            let mut events = PortEvent::none();
            events.status.set_plug_inserted_or_removed(true);
            self.events.signal(events);
        }

        /// Simulate a debug accessory source connecting
        pub async fn connect_debug_accessory_source(&self, current: Current) {
            self.connect(Contract::Sink(current.into()), true).await;
        }

        /// Simulate a PD alert
        pub async fn send_pd_alert(&self, ado: Ado) {
            *self.pd_alert.lock().await = Some(ado);

            let mut events = PortEvent::none();
            events.notification.set_alert(true);
            self.events.signal(events);
        }
    }

    pub struct Controller<'a> {
        state: &'a ControllerState,
        events: Cell<PortEvent>,
    }

    impl<'a> Controller<'a> {
        pub fn new(state: &'a ControllerState) -> Self {
            Self {
                state,
                events: Cell::new(PortEvent::none()),
            }
        }

        /// Function to demonstrate calling functions directly on the controller
        pub fn custom_function(&self) {
            info!("Custom function called on controller");
        }
    }

    impl embedded_services::type_c::controller::Controller for Controller<'_> {
        type BusError = ();

        async fn sync_state(&mut self) -> Result<(), Error<Self::BusError>> {
            Ok(())
        }

        async fn wait_port_event(&mut self) -> Result<(), Error<Self::BusError>> {
            let events = self.state.events.wait().await;
            trace!("Port event: {events:#?}");
            self.events.set(events);
            Ok(())
        }

        async fn clear_port_events(&mut self, _port: LocalPortId) -> Result<PortEvent, Error<Self::BusError>> {
            let events = self.events.get();
            debug!("Clear port events: {events:#?}");
            self.events.set(PortEvent::none());
            Ok(events)
        }

        async fn get_port_status(
            &mut self,
            _port: LocalPortId,
            _cached: bool,
        ) -> Result<PortStatus, Error<Self::BusError>> {
            debug!("Get port status: {:#?}", *self.state.status.lock().await);
            Ok(*self.state.status.lock().await)
        }

        async fn enable_sink_path(&mut self, _port: LocalPortId, enable: bool) -> Result<(), Error<Self::BusError>> {
            debug!("Enable sink path: {enable}");
            Ok(())
        }

        async fn get_controller_status(&mut self) -> Result<ControllerStatus<'static>, Error<Self::BusError>> {
            debug!("Get controller status");
            Ok(ControllerStatus {
                mode: "Test",
                valid_fw_bank: true,
                fw_version0: 0xbadf00d,
                fw_version1: 0xdeadbeef,
            })
        }

        async fn get_rt_fw_update_status(
            &mut self,
            _port: LocalPortId,
        ) -> Result<RetimerFwUpdateState, Error<Self::BusError>> {
            debug!("Get retimer fw update status");
            Ok(RetimerFwUpdateState::Inactive)
        }

        async fn set_rt_fw_update_state(&mut self, _port: LocalPortId) -> Result<(), Error<Self::BusError>> {
            debug!("Set retimer fw update state");
            Ok(())
        }

        async fn clear_rt_fw_update_state(&mut self, _port: LocalPortId) -> Result<(), Error<Self::BusError>> {
            debug!("Clear retimer fw update state");
            Ok(())
        }

        async fn set_rt_compliance(&mut self, _port: LocalPortId) -> Result<(), Error<Self::BusError>> {
            debug!("Set retimer compliance");
            Ok(())
        }

        async fn get_pd_alert(&mut self, port: LocalPortId) -> Result<Option<Ado>, Error<Self::BusError>> {
            let pd_alert = self.state.pd_alert.lock().await;
            if let Some(ado) = *pd_alert {
                debug!("Port{}: Get PD alert: {ado:#?}", port.0);
                Ok(Some(ado))
            } else {
                debug!("Port{}: No PD alert", port.0);
                Ok(None)
            }
        }

        async fn get_active_fw_version(&self) -> Result<u32, Error<Self::BusError>> {
            Ok(0)
        }

        async fn start_fw_update(&mut self) -> Result<(), Error<Self::BusError>> {
            Ok(())
        }

        async fn abort_fw_update(&mut self) -> Result<(), Error<Self::BusError>> {
            Ok(())
        }

        async fn finalize_fw_update(&mut self) -> Result<(), Error<Self::BusError>> {
            Ok(())
        }

        async fn write_fw_contents(&mut self, _offset: usize, _data: &[u8]) -> Result<(), Error<Self::BusError>> {
            Ok(())
        }
    }

    pub struct Validator;

    impl type_c_service::wrapper::FwOfferValidator for Validator {
        fn validate(
            &self,
            _current: embedded_cfu_protocol::protocol_definitions::FwVersion,
            _offer: &embedded_cfu_protocol::protocol_definitions::FwUpdateOffer,
        ) -> embedded_cfu_protocol::protocol_definitions::FwUpdateOfferResponse {
            // For this example, we always accept the new version
            FwUpdateOfferResponse::new_accept(HostToken::Driver)
        }
    }

    pub type Wrapper<'a> = type_c_service::wrapper::ControllerWrapper<'a, 1, Controller<'a>, Validator>;
}

mod debug {
    use embedded_services::{
        comms::{self, Endpoint, EndpointID, Internal},
        info,
        type_c::comms::DebugAccessoryMessage,
    };

    pub struct Listener {
        pub tp: Endpoint,
    }

    impl Listener {
        pub fn new() -> Self {
            Self {
                tp: Endpoint::uninit(EndpointID::Internal(Internal::Usbc)),
            }
        }
    }

    impl comms::MailboxDelegate for Listener {
        fn receive(&self, message: &comms::Message) -> Result<(), comms::MailboxDelegateError> {
            if let Some(message) = message.data.get::<DebugAccessoryMessage>() {
                if message.connected {
                    info!("Port{}: Debug accessory connected", message.port.0);
                } else {
                    info!("Port{}: Debug accessory disconnected", message.port.0);
                }
            }

            Ok(())
        }
    }
}

#[embassy_executor::task]
async fn controller_task(state: &'static test_controller::ControllerState) {
    static WRAPPER: OnceLock<test_controller::Wrapper> = OnceLock::new();

    let controller = test_controller::Controller::new(state);
    let wrapper = WRAPPER.get_or_init(|| {
        test_controller::Wrapper::new(
            embedded_services::type_c::controller::Device::new(CONTROLLER0, &[PORT0, PORT0]),
            [policy::device::Device::new(POWER0)],
            embedded_services::cfu::component::CfuDevice::new(0x00),
            controller,
            crate::test_controller::Validator,
        )
    });

    wrapper.register().await.unwrap();

    wrapper.get_inner().await.custom_function();

    loop {
        let event = wrapper.wait_next().await;
        if let Err(e) = event {
            error!("Error waiting for event: {e:?}");
            continue;
        }

        let event = event.unwrap();
        if let Event::PdAlert(port_id, ado) = event {
            info!("Port{}: PD alert received: {:?}", port_id.0, ado);
        }

        if let Err(e) = wrapper.process_next_event(event).await {
            error!("Error processing event: {e:?}");
        }
    }
}

#[embassy_executor::task]
async fn task(spawner: Spawner) {
    embedded_services::init().await;

    controller::init();

    // Register debug accessory listener
    static LISTENER: OnceLock<debug::Listener> = OnceLock::new();
    let listener = LISTENER.get_or_init(debug::Listener::new);
    comms::register_endpoint(listener, &listener.tp).await.unwrap();

    static STATE: OnceLock<test_controller::ControllerState> = OnceLock::new();
    let state = STATE.get_or_init(test_controller::ControllerState::new);

    info!("Starting controller task");
    spawner.must_spawn(controller_task(state));
    // Wait for controller to be registered
    Timer::after_secs(1).await;

    info!("Simulating connection");
    state.connect_sink(Current::UsbDefault).await;
    Timer::after_millis(DELAY_MS).await;

    info!("Simulating PD alert");
    state.send_pd_alert(Ado::PowerButtonPress).await;
    Timer::after_millis(DELAY_MS).await;

    info!("Simulating disconnection");
    state.disconnect().await;
    Timer::after_millis(DELAY_MS).await;

    info!("Simulating debug accessory connection");
    state.connect_debug_accessory_source(Current::UsbDefault).await;
    Timer::after_millis(DELAY_MS).await;

    info!("Simulating debug accessory disconnection");
    state.disconnect().await;
    Timer::after_millis(DELAY_MS).await;
}

fn main() {
    env_logger::builder().filter_level(log::LevelFilter::Trace).init();

    static EXECUTOR: StaticCell<Executor> = StaticCell::new();
    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.must_spawn(power_policy_service::task(Default::default()));
        spawner.must_spawn(type_c_service::task());
        spawner.must_spawn(task(spawner));
    });
}
