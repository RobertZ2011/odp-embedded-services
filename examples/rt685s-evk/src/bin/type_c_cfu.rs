#![no_std]
#![no_main]

use ::tps6699x::ADDR1;
use defmt::info;
use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_imxrt::gpio::{Input, Inverter, Pull};
use embassy_imxrt::i2c::master::{Config, I2cMaster};
use embassy_imxrt::i2c::Async;
use embassy_imxrt::{bind_interrupts, peripherals};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_sync::once_lock::OnceLock;
use embassy_time::{self as _, Delay};
use static_cell::StaticCell;
use embedded_services::cfu::bridge::MessageBridge;
use embedded_services::cfu::component::{CfuDevice};
use embedded_services::cfu::component::RequestData;
use embedded_cfu_protocol::protocol_definitions::*;
use embedded_services::cfu::component::InternalResponseData;
use embedded_services::cfu;
use embassy_time::Timer;
use tps6699x::asynchronous::embassy as tps6699x;
use type_c_service::driver::tps6699x::{self as tps6699x_drv};

extern crate rt685s_evk_example;

const CONTROLLER0_CFU_ID: ComponentId = 0;

bind_interrupts!(struct Irqs {
    FLEXCOMM2 => embassy_imxrt::i2c::InterruptHandler<peripherals::FLEXCOMM2>;
});

type BusMaster<'a> = I2cMaster<'a, Async>;
type BusDevice<'a> = I2cDevice<'a, NoopRawMutex, BusMaster<'a>>;
type Wrapper<'a> = MessageBridge<tps6699x_drv::Tps6699x<'a, 2, NoopRawMutex, BusDevice<'a>>>;
type Controller<'a> = tps6699x::controller::Controller<NoopRawMutex, BusDevice<'a>>;
type Interrupt<'a> = tps6699x::Interrupt<'a, NoopRawMutex, BusDevice<'a>>;

#[embassy_executor::task]
async fn pd_controller_task(controller: &'static Wrapper<'static>) {
    loop {
        controller.process().await;
    }
}

#[embassy_executor::task]
async fn interrupt_task(mut int_in: Input<'static>, mut interrupt: Interrupt<'static>) {
    tps6699x::task::interrupt_task(&mut int_in, &mut [&mut interrupt]).await;
}

#[embassy_executor::task]
async fn fw_update_task() {
    Timer::after_millis(1000).await;
    let context = cfu::ContextToken::create().unwrap();
    let device = context.get_device(CONTROLLER0_CFU_ID).await.unwrap();

    info!("Getting FW version");
    let response = device.execute_device_request(RequestData::FwVersionRequest).await.unwrap();
    let prev_version = match response {
        InternalResponseData::FwVersionResponse(GetFwVersionResponse { component_info, .. }) => {
            Into::<u32>::into(component_info[0].fw_version)
        }
        _ => panic!("Unexpected response"),
    };
    info!("Got version: {:#x}", prev_version);

    info!("Giving offer");
    let offer = device.execute_device_request(RequestData::GiveOffer(FwUpdateOffer::new(HostToken::Driver, CONTROLLER0_CFU_ID, FwVersion::default(), 0, 0))).await.unwrap();
    info!("Got response: {:?}", offer);

    let fw = include_bytes!("../../885_MIS-TCPM0-0.0.1.bin");
    let num_chunks = fw.len() / DEFAULT_DATA_LENGTH;

    for (i, chunk) in fw.chunks(DEFAULT_DATA_LENGTH).enumerate() {
        let header = FwUpdateContentHeader {
            data_length: chunk.len() as u8,
            sequence_num: i as u16,
            firmware_address: (i * DEFAULT_DATA_LENGTH) as u32,
            flags: if i == 0 {
                FW_UPDATE_FLAG_FIRST_BLOCK
            } else if i == num_chunks - 1 {
                FW_UPDATE_FLAG_LAST_BLOCK
            } else {
                0
            }
        };
        
        let mut chunk_data = [0u8; DEFAULT_DATA_LENGTH];
        chunk_data[..chunk.len()].copy_from_slice(chunk);
        let request = FwUpdateContentCommand {
            header,
            data: chunk_data,
        };

        info!("Sending chunk {} of {}", i, fw.len());
        let response = device.execute_device_request(RequestData::GiveContent(request)).await.unwrap();
        info!("Got response: {:?}", response);
    }

    device.execute_device_request(RequestData::FinalizeUpdate).await.unwrap();

    Timer::after_millis(2000).await;
    info!("Getting FW version");
    let response = device.execute_device_request(RequestData::FwVersionRequest).await.unwrap();
    let version = match response {
        InternalResponseData::FwVersionResponse(GetFwVersionResponse { component_info, .. }) => {
            Into::<u32>::into(component_info[0].fw_version)
        }
        _ => panic!("Unexpected response"),
    };
    info!("Got previous version: {:#x}", prev_version);
    info!("Got version: {:#x}",version);
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_imxrt::init(Default::default());

    info!("Embedded service init");
    embedded_services::init().await;

    let int_in = Input::new(p.PIO1_7, Pull::Up, Inverter::Disabled);
    static BUS: OnceLock<Mutex<NoopRawMutex, BusMaster<'static>>> = OnceLock::new();
    let bus = BUS.get_or_init(|| {
        Mutex::new(
            I2cMaster::new_async(p.FLEXCOMM2, p.PIO0_18, p.PIO0_17, Irqs, Config::default(), p.DMA0_CH5).unwrap(),
        )
    });

    let device = I2cDevice::new(bus);

    static CONTROLLER: StaticCell<Controller<'static>> = StaticCell::new();
    let controller = CONTROLLER.init(Controller::new_tps66994(device, ADDR1).unwrap());
    let (mut tps6699x, interrupt) = controller.make_parts();

    info!("Resetting PD controller");
    let mut delay = Delay;
    tps6699x.reset(&mut delay).await.unwrap();

    info!("Spawining interrupt task");
    spawner.must_spawn(interrupt_task(int_in, interrupt));

    info!("Spawining PD controller task");
    static PD_CONTROLLER: OnceLock<Wrapper> = OnceLock::new();
    let pd_controller = PD_CONTROLLER.get_or_init(|| {
        MessageBridge::new(CfuDevice::new(CONTROLLER0_CFU_ID), tps6699x_drv::Tps6699x::new(tps6699x))
    });

    cfu::register_device(pd_controller).await.unwrap();
    info!("Spawining PD controller task");
    spawner.must_spawn(pd_controller_task(pd_controller));

    info!("Spawining FW update task");
    spawner.must_spawn(fw_update_task());
}
