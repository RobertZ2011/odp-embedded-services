use crate::mock_controller::Wrapper;
use embassy_executor::{Executor, Spawner};
use embassy_sync::channel::Channel;
use embassy_sync::channel::DynamicReceiver;
use embassy_sync::channel::DynamicSender;
use embassy_sync::mutex::Mutex;
use embassy_sync::pubsub::{DynImmediatePublisher, DynSubscriber, PubSubChannel};
use embassy_time::Timer;
use embedded_services::GlobalRawMutex;
use embedded_services::event::MapSender;
use embedded_usb_pd::{GlobalPortId, LocalPortId};
use log::*;
use power_policy_interface::capability::PowerCapability;
use power_policy_interface::charger::mock::ChargerType;
use power_policy_interface::psu;
use power_policy_service::psu::PsuEventReceivers;
use power_policy_service::service::registration::ArrayRegistration;
use static_cell::StaticCell;
use std_examples::type_c::mock_controller::{self, InterruptReceiver};
use type_c_interface::port::ControllerId;
use type_c_interface::port::PortRegistration;
use type_c_interface::port::event::PortEventBitfield;
use type_c_interface::service::event::PortEvent as ServicePortEvent;
use type_c_service::bridge::Bridge;
use type_c_service::bridge::event_receiver::EventReceiver as BridgeEventReceiver;
use type_c_service::service::{EventReceiver as ServiceEventReceiver, Service};
use type_c_service::wrapper::backing::{IntermediateStorage, ReferencedStorage, Storage};
use type_c_service::wrapper::proxy::PowerProxyDevice;
use type_c_service::wrapper::proxy::event::Event as PortEvent;
use type_c_service::wrapper::proxy::event_receiver::InterruptReceiver as _;
use type_c_service::wrapper::proxy::event_receiver::{EventReceiver as PortEventReceiver, PortEventSplitter};
use type_c_service::wrapper::proxy::state::SharedState;

const CHANNEL_CAPACITY: usize = 4;

const NUM_PD_CONTROLLERS: usize = 3;

const CONTROLLER0_ID: ControllerId = ControllerId(0);
const PORT0_ID: GlobalPortId = GlobalPortId(0);

const CONTROLLER1_ID: ControllerId = ControllerId(1);
const PORT1_ID: GlobalPortId = GlobalPortId(1);

const CONTROLLER2_ID: ControllerId = ControllerId(2);
const PORT2_ID: GlobalPortId = GlobalPortId(2);

const DELAY_MS: u64 = 1000;

type ControllerType = Mutex<GlobalRawMutex, mock_controller::Controller<'static>>;
type DeviceType = Mutex<GlobalRawMutex, PowerProxyDevice<'static, ControllerType>>;

type PowerPolicySenderType = MapSender<
    power_policy_interface::service::event::Event<'static, DeviceType>,
    power_policy_interface::service::event::EventData,
    DynImmediatePublisher<'static, power_policy_interface::service::event::EventData>,
    fn(
        power_policy_interface::service::event::Event<'static, DeviceType>,
    ) -> power_policy_interface::service::event::EventData,
>;

type PowerPolicyReceiverType = DynSubscriber<'static, power_policy_interface::service::event::EventData>;

type PowerPolicyServiceType = Mutex<
    GlobalRawMutex,
    power_policy_service::service::Service<
        'static,
        ArrayRegistration<'static, DeviceType, 3, PowerPolicySenderType, 1, ChargerType, 0>,
    >,
>;

type ServiceType = Service<'static>;
type SharedStateType = Mutex<GlobalRawMutex, SharedState>;
type PortEventReceiverType = PortEventReceiver<'static, SharedStateType, DynamicReceiver<'static, PortEventBitfield>>;

#[embassy_executor::task(pool_size = 3)]
async fn bridge_task(
    mut event_receiver: BridgeEventReceiver,
    mut bridge: Bridge<'static, Mutex<GlobalRawMutex, mock_controller::Controller<'static>>>,
) -> ! {
    loop {
        let event = event_receiver.wait_next().await;
        let output = bridge.process_event(event).await;
        event_receiver.finalize(output);
    }
}

#[embassy_executor::task(pool_size = 3)]
async fn controller_task(
    shared_state: &'static SharedStateType,
    mut event_receiver: PortEventReceiverType,
    wrapper: &'static mock_controller::Wrapper<'static>,
) {
    loop {
        let PortEvent::PortEvent(event) = event_receiver.wait_event().await;

        let mut shared_state = shared_state.lock().await;
        let output = wrapper
            .process_event(
                &mut shared_state,
                type_c_service::wrapper::message::Event::PortEvent(type_c_service::wrapper::message::LocalPortEvent {
                    port: LocalPortId(0),
                    event,
                }),
            )
            .await;
        if let Err(e) = output {
            error!("Error processing event: {e:?}");
        }
        let output = output.unwrap();
        if let Err(e) = wrapper.finalize(output).await {
            error!("Error finalizing output: {e:#?}");
        }
    }
}

#[embassy_executor::task(pool_size = 3)]
async fn interrupt_splitter_task(
    mut interrupt_receiver: InterruptReceiver<'static>,
    mut interrupt_splitter: PortEventSplitter<1, DynamicSender<'static, PortEventBitfield>>,
) -> ! {
    loop {
        let interrupts = interrupt_receiver.wait_interrupt().await;
        interrupt_splitter.process_interrupts(interrupts).await;
    }
}

#[embassy_executor::task]
async fn task(spawner: Spawner) {
    embedded_services::init().await;

    // Create power policy service
    static CONTROLLER_CONTEXT: StaticCell<type_c_interface::service::context::Context> = StaticCell::new();
    let controller_context = CONTROLLER_CONTEXT.init(type_c_interface::service::context::Context::new());

    static STATE0: StaticCell<mock_controller::ControllerState> = StaticCell::new();
    let state0 = STATE0.init(mock_controller::ControllerState::new());
    static CONTROLLER0: StaticCell<ControllerType> = StaticCell::new();
    let controller0 = CONTROLLER0.init(Mutex::new(mock_controller::Controller::new(state0)));

    static PORT0_CHANNEL: Channel<GlobalRawMutex, ServicePortEvent, CHANNEL_CAPACITY> = Channel::new();
    static STORAGE0: StaticCell<Storage<1>> = StaticCell::new();
    let storage0 = STORAGE0.init(Storage::new(
        controller_context,
        CONTROLLER0_ID,
        [PortRegistration {
            id: PORT0_ID,
            sender: PORT0_CHANNEL.dyn_sender(),
            receiver: PORT0_CHANNEL.dyn_receiver(),
        }],
    ));

    static POLICY_CHANNEL0: StaticCell<Channel<GlobalRawMutex, psu::event::EventData, 1>> = StaticCell::new();
    let policy_channel0 = POLICY_CHANNEL0.init(Channel::new());
    let policy_sender0 = policy_channel0.dyn_sender();
    let policy_receiver0 = policy_channel0.dyn_receiver();

    static INTERMEDIATE0: StaticCell<
        IntermediateStorage<1, GlobalRawMutex, ControllerType, DynamicSender<'static, psu::event::EventData>>,
    > = StaticCell::new();
    let intermediate0 = storage0
        .try_create_intermediate([("Pd0", LocalPortId(0), controller0, policy_sender0)])
        .expect("Failed to create intermediate storage");
    let intermediate0 = INTERMEDIATE0.init(intermediate0);

    static REFERENCED0: StaticCell<ReferencedStorage<1, GlobalRawMutex, ControllerType, DynamicSender<'_, psu::event::EventData>>> =
        StaticCell::new();
    let referenced0 = REFERENCED0.init(
        intermediate0
            .try_create_referenced()
            .expect("Failed to create referenced storage"),
    );

    static PORT_SHARED_STATE_0: StaticCell<SharedStateType> = StaticCell::new();
    let port_shared_state_0 = PORT_SHARED_STATE_0.init(Mutex::new(SharedState::new()));
    static PORT_INTERRUPT_CHANNEL_0: StaticCell<Channel<GlobalRawMutex, PortEventBitfield, CHANNEL_CAPACITY>> =
        StaticCell::new();
    let port_interrupt_channel_0 = PORT_INTERRUPT_CHANNEL_0.init(Channel::new());
    let port_interrupt_receiver_0 = port_interrupt_channel_0.dyn_receiver();
    let port_interrupt_sender_0 = port_interrupt_channel_0.dyn_sender();

    static WRAPPER0: StaticCell<mock_controller::Wrapper> = StaticCell::new();
    let wrapper0 = WRAPPER0.init(mock_controller::Wrapper::new(
        controller0,
        Default::default(),
        referenced0,
    ));
    let bridge_receiver0 = BridgeEventReceiver::new(&referenced0.pd_controller);
    let bridge0 = Bridge::new(controller0, &referenced0.pd_controller);

    static STATE1: StaticCell<mock_controller::ControllerState> = StaticCell::new();
    let state1 = STATE1.init(mock_controller::ControllerState::new());
    static CONTROLLER1: StaticCell<ControllerType> = StaticCell::new();
    let controller1 = CONTROLLER1.init(Mutex::new(mock_controller::Controller::new(state1)));

    static PORT1_CHANNEL: Channel<GlobalRawMutex, ServicePortEvent, CHANNEL_CAPACITY> = Channel::new();
    static STORAGE1: StaticCell<Storage<1>> = StaticCell::new();
    let storage1 = STORAGE1.init(Storage::new(
        controller_context,
        CONTROLLER1_ID,
        [PortRegistration {
            id: PORT1_ID,
            sender: PORT1_CHANNEL.dyn_sender(),
            receiver: PORT1_CHANNEL.dyn_receiver(),
        }],
    ));

    static POLICY_CHANNEL1: StaticCell<Channel<GlobalRawMutex, psu::event::EventData, 1>> = StaticCell::new();
    let policy_channel1 = POLICY_CHANNEL1.init(Channel::new());
    let policy_sender1 = policy_channel1.dyn_sender();
    let policy_receiver1 = policy_channel1.dyn_receiver();

    static INTERMEDIATE1: StaticCell<
        IntermediateStorage<1, GlobalRawMutex, ControllerType, DynamicSender<'static, psu::event::EventData>>,
    > = StaticCell::new();
    let intermediate1 = storage1
        .try_create_intermediate([("Pd1", LocalPortId(0), controller1, policy_sender1)])
        .expect("Failed to create intermediate storage");
    let intermediate1 = INTERMEDIATE1.init(intermediate1);

    static REFERENCED1: StaticCell<ReferencedStorage<1, GlobalRawMutex, ControllerType, DynamicSender<'_, psu::event::EventData>>> =
        StaticCell::new();
    let referenced1 = REFERENCED1.init(
        intermediate1
            .try_create_referenced()
            .expect("Failed to create referenced storage"),
    );

    static PORT_SHARED_STATE_1: StaticCell<SharedStateType> = StaticCell::new();
    let port_shared_state_1 = PORT_SHARED_STATE_1.init(Mutex::new(SharedState::new()));
    static PORT_INTERRUPT_CHANNEL_1: StaticCell<Channel<GlobalRawMutex, PortEventBitfield, CHANNEL_CAPACITY>> =
        StaticCell::new();
    let port_interrupt_channel_1 = PORT_INTERRUPT_CHANNEL_1.init(Channel::new());
    let port_interrupt_receiver_1 = port_interrupt_channel_1.dyn_receiver();
    let port_interrupt_sender_1 = port_interrupt_channel_1.dyn_sender();

    static WRAPPER1: StaticCell<mock_controller::Wrapper> = StaticCell::new();
    let wrapper1 = WRAPPER1.init(mock_controller::Wrapper::new(
        controller1,
        Default::default(),
        referenced1,
    ));
    let bridge_receiver1 = BridgeEventReceiver::new(&referenced1.pd_controller);
    let bridge1 = Bridge::new(controller1, &referenced1.pd_controller);

    static STATE2: StaticCell<mock_controller::ControllerState> = StaticCell::new();
    let state2 = STATE2.init(mock_controller::ControllerState::new());
    static CONTROLLER2: StaticCell<ControllerType> = StaticCell::new();
    let controller2 = CONTROLLER2.init(Mutex::new(mock_controller::Controller::new(state2)));

    static PORT2_CHANNEL: Channel<GlobalRawMutex, ServicePortEvent, CHANNEL_CAPACITY> = Channel::new();
    static STORAGE2: StaticCell<Storage<1>> = StaticCell::new();
    let storage2 = STORAGE2.init(Storage::new(
        controller_context,
        CONTROLLER2_ID,
        [PortRegistration {
            id: PORT2_ID,
            sender: PORT2_CHANNEL.dyn_sender(),
            receiver: PORT2_CHANNEL.dyn_receiver(),
        }],
    ));

    static POLICY_CHANNEL2: StaticCell<Channel<GlobalRawMutex, psu::event::EventData, 1>> = StaticCell::new();
    let policy_channel2 = POLICY_CHANNEL2.init(Channel::new());
    let policy_sender2 = policy_channel2.dyn_sender();
    let policy_receiver2 = policy_channel2.dyn_receiver();

    static INTERMEDIATE2: StaticCell<
        IntermediateStorage<1, GlobalRawMutex, ControllerType, DynamicSender<'static, psu::event::EventData>>,
    > = StaticCell::new();
    let intermediate2 = storage2
        .try_create_intermediate([("Pd2", LocalPortId(0), controller2, policy_sender2)])
        .expect("Failed to create intermediate storage");
    let intermediate2 = INTERMEDIATE2.init(intermediate2);

    static REFERENCED2: StaticCell<ReferencedStorage<1, GlobalRawMutex, ControllerType, DynamicSender<'_, psu::event::EventData>>> =
        StaticCell::new();
    let referenced2 = REFERENCED2.init(
        intermediate2
            .try_create_referenced()
            .expect("Failed to create referenced storage"),
    );

    static PORT_SHARED_STATE_2: StaticCell<SharedStateType> = StaticCell::new();
    let port_shared_state_2 = PORT_SHARED_STATE_2.init(Mutex::new(SharedState::new()));
    static PORT_INTERRUPT_CHANNEL_2: StaticCell<Channel<GlobalRawMutex, PortEventBitfield, CHANNEL_CAPACITY>> =
        StaticCell::new();
    let port_interrupt_channel_2 = PORT_INTERRUPT_CHANNEL_2.init(Channel::new());
    let port_interrupt_receiver_2 = port_interrupt_channel_2.dyn_receiver();
    let port_interrupt_sender_2 = port_interrupt_channel_2.dyn_sender();

    static WRAPPER2: StaticCell<mock_controller::Wrapper> = StaticCell::new();
    let wrapper2 = WRAPPER2.init(mock_controller::Wrapper::new(
        controller2,
        Default::default(),
        referenced2,
    ));
    let bridge_receiver2 = BridgeEventReceiver::new(&referenced2.pd_controller);
    let bridge2 = Bridge::new(controller2, &referenced2.pd_controller);

    // The service is the only receiver and we only use a DynImmediatePublisher, which doesn't take a publisher slot
    static POWER_POLICY_CHANNEL: StaticCell<
        PubSubChannel<GlobalRawMutex, power_policy_interface::service::event::EventData, 4, 1, 0>,
    > = StaticCell::new();

    let power_policy_channel = POWER_POLICY_CHANNEL.init(PubSubChannel::new());
    let power_policy_sender: PowerPolicySenderType =
        MapSender::new(power_policy_channel.dyn_immediate_publisher(), |e| e.into());
    // Guaranteed to not panic since we initialized the channel above
    let power_policy_subscriber = power_policy_channel.dyn_subscriber().unwrap();

    let power_policy_registration = ArrayRegistration {
        psus: [
            &wrapper0.ports[0].proxy,
            &wrapper1.ports[0].proxy,
            &wrapper2.ports[0].proxy,
        ],
        service_senders: [power_policy_sender],
        chargers: [],
    };

    static POWER_SERVICE: StaticCell<PowerPolicyServiceType> = StaticCell::new();
    let power_service = POWER_SERVICE.init(Mutex::new(power_policy_service::service::Service::new(
        power_policy_registration,
        power_policy_service::service::config::Config::default(),
    )));

    // Create type-c service
    static TYPE_C_SERVICE: StaticCell<Mutex<GlobalRawMutex, ServiceType>> = StaticCell::new();
    let type_c_service = TYPE_C_SERVICE.init(Mutex::new(Service::create(Default::default(), controller_context)));

    spawner.spawn(
        power_policy_task(
            PsuEventReceivers::new(
                [
                    &wrapper0.ports[0].proxy,
                    &wrapper1.ports[0].proxy,
                    &wrapper2.ports[0].proxy,
                ],
                [policy_receiver0, policy_receiver1, policy_receiver2],
            ),
            power_service,
        )
        .expect("Failed to create power policy task"),
    );
    spawner.spawn(
        type_c_service_task(
            type_c_service,
            ServiceEventReceiver::new(controller_context, power_policy_subscriber),
            [wrapper0, wrapper1, wrapper2],
        )
        .expect("Failed to create type-c service task"),
    );

    spawner.spawn(bridge_task(bridge_receiver0, bridge0).expect("Failed to create bridge0 task"));
    spawner.spawn(bridge_task(bridge_receiver1, bridge1).expect("Failed to create bridge1 task"));
    spawner.spawn(bridge_task(bridge_receiver2, bridge2).expect("Failed to create bridge2 task"));
    spawner.spawn(
        controller_task(
            port_shared_state_0,
            PortEventReceiver::new(port_shared_state_0, port_interrupt_receiver_0),
            wrapper0,
        )
        .expect("Failed to create controller0 task"),
    );
    spawner.spawn(
        interrupt_splitter_task(
            state0.create_interrupt_receiver(),
            PortEventSplitter::new([port_interrupt_sender_0]),
        )
        .expect("Failed to create interrupt splitter 0 task"),
    );
    spawner.spawn(
        controller_task(
            port_shared_state_1,
            PortEventReceiver::new(port_shared_state_1, port_interrupt_receiver_1),
            wrapper1,
        )
        .expect("Failed to create controller1 task"),
    );
    spawner.spawn(
        interrupt_splitter_task(
            state1.create_interrupt_receiver(),
            PortEventSplitter::new([port_interrupt_sender_1]),
        )
        .expect("Failed to create interrupt splitter 1 task"),
    );
    spawner.spawn(
        controller_task(
            port_shared_state_2,
            PortEventReceiver::new(port_shared_state_2, port_interrupt_receiver_2),
            wrapper2,
        )
        .expect("Failed to create controller2 task"),
    );
    spawner.spawn(
        interrupt_splitter_task(
            state2.create_interrupt_receiver(),
            PortEventSplitter::new([port_interrupt_sender_2]),
        )
        .expect("Failed to create interrupt splitter 2 task"),
    );

    const CAPABILITY: PowerCapability = PowerCapability {
        voltage_mv: 20000,
        current_ma: 5000,
    };

    // Wait for controller to be registered
    Timer::after_secs(1).await;

    info!("Connecting port 0, unconstrained");
    state0.connect_sink(CAPABILITY, true).await;
    Timer::after_millis(DELAY_MS).await;

    info!("Connecting port 1, constrained");
    state1.connect_sink(CAPABILITY, false).await;
    Timer::after_millis(DELAY_MS).await;

    info!("Disconnecting port 0");
    state0.disconnect().await;
    Timer::after_millis(DELAY_MS).await;

    info!("Disconnecting port 1");
    state1.disconnect().await;
    Timer::after_millis(DELAY_MS).await;

    info!("Connecting port 0, unconstrained");
    state0.connect_sink(CAPABILITY, true).await;
    Timer::after_millis(DELAY_MS).await;

    info!("Connecting port 1, unconstrained");
    state1.connect_sink(CAPABILITY, true).await;
    Timer::after_millis(DELAY_MS).await;

    info!("Connecting port 2, unconstrained");
    state2.connect_sink(CAPABILITY, true).await;
    Timer::after_millis(DELAY_MS).await;

    info!("Disconnecting port 0");
    state0.disconnect().await;
    Timer::after_millis(DELAY_MS).await;

    info!("Disconnecting port 1");
    state1.disconnect().await;
    Timer::after_millis(DELAY_MS).await;

    info!("Disconnecting port 2");
    state2.disconnect().await;
    Timer::after_millis(DELAY_MS).await;
}

#[embassy_executor::task]
async fn power_policy_task(
    psu_events: PsuEventReceivers<'static, 3, DeviceType, DynamicReceiver<'static, psu::event::EventData>>,
    power_policy: &'static PowerPolicyServiceType,
) {
    power_policy_service::service::task::psu_task(psu_events, power_policy).await;
}

#[embassy_executor::task]
async fn type_c_service_task(
    service: &'static Mutex<GlobalRawMutex, ServiceType>,
    event_receiver: ServiceEventReceiver<'static, PowerPolicyReceiverType>,
    wrappers: [&'static Wrapper<'static>; NUM_PD_CONTROLLERS],
) {
    info!("Starting type-c task");
    type_c_service::task::task(service, event_receiver, wrappers).await;
}

fn main() {
    env_logger::builder().filter_level(log::LevelFilter::Trace).init();

    static EXECUTOR: StaticCell<Executor> = StaticCell::new();
    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(task(spawner).expect("Failed to create task"));
    });
}
